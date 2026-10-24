//! Be A Legend, the League Challenge and their progression. The game's own
//! numbers, traced from `SLUS_206.51` (`docs/TRACE.md` §5, "Mode tables"):
//! the event table at 0x460790, the reputation tables at 0x460b28 and
//! 0x460b70, the development-point charts at 0x460a88/0x460af8/0x460b10, the league
//! Challenge region table at 0x3ec9e8 and the cost bands at 0x4604c8.
//! Where the game's own reading was not pinned down, the comment says
//! `likely` — those spots are listed to trace in `docs/TRACE.md` §8.
//!
//! A [`Story`] is pure data. The game keeps one per save file and advances
//! it after each match.

/// How an event becomes visible or playable (the `+0x0c`/`+0x10` words:
/// bit 15 set means "clear that event first", else overall points needed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Gate {
    /// Overall progress points needed.
    Points(u32),
    /// Event `u16` (without bit 15) must be cleared first.
    Clear(u16),
    /// Always.
    Now,
}

impl Gate {
    /// The gate's own word, as the table stores it.
    pub fn word(self) -> u32 {
        match self {
            Gate::Points(n) => n,
            Gate::Clear(ev) => 0x8000 | u32::from(ev),
            Gate::Now => 0,
        }
    }
}

const fn from_word(word: u32) -> Gate {
    if word & 0x8000 != 0 {
        Gate::Clear((word & 0x7fff) as u16)
    } else {
        Gate::Points(word)
    }
}

/// What kind of event a row is (`+0x00`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    Pickup,
    Tournament,
    StreetChallenge,
}

/// A tournament's shape (`+0x1b`, through `FUN_001eae48`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Format {
    /// Single elimination (three rounds).
    SingleElimination,
    /// Double elimination (four rounds).
    DoubleElimination,
    /// Best of three.
    BestOfThree,
}

/// One row of the event table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Event {
    pub kind: EventKind,
    /// The XDBCOURT record (`+0x04`) — the venue, and the city anchor.
    pub court: u16,
    /// The row's payout tier 0..4 (`+0x08`).
    pub level: u8,
    pub visible: Gate,
    pub open: Gate,
    /// Progress points to clear a pick-up (`+0x14`; -1 for the rest).
    pub target: i16,
    /// Pick-up progress points per game (`+0x18`).
    pub points_per_game: u8,
    /// A tournament's boss team (XDBTEAM 40..45, `+0x1a`); a street
    /// challenge's unlock id (0x41..0x46, display); 0xff for no boss (`likely`,
    /// from the '78 row's word).
    pub boss_team: u16,
    pub format: Format,
    /// `+0x1b`: a tournament's format row (`0x460a10`, `FUN_001ead68`), a
    /// street challenge's rules row ([`STREET_RULES`], `FUN_001e91a0`); 0
    /// for a pick-up.
    pub row: u8,
}

#[allow(clippy::too_many_arguments)]
const fn ev(
    kind: EventKind,
    court: u16,
    level: u8,
    visible: u32,
    open: u32,
    target: i16,
    per_game: u8,
    boss: u16,
    row: u8,
) -> Event {
    // The tournaments' rows at `0x460a10`: 0 single elimination (three
    // rounds), 2 double elimination (four), 4 best of three.
    let format = match (kind, row) {
        (EventKind::Tournament, 2) => Format::DoubleElimination,
        (EventKind::Tournament, 4) => Format::BestOfThree,
        _ => Format::SingleElimination,
    };
    Event {
        kind,
        court,
        level,
        visible: from_word(visible),
        open: from_word(open),
        target,
        points_per_game: per_game,
        boss_team: boss,
        format,
        row,
    }
}

/// The event table, in record order: 0..10 the map's challen­ges (pick-ups,
/// tournaments and the Rucker Park '78 challenge) and 11..16 the six rule
/// variants sitting on the same courts.
pub const EVENTS: [Event; 17] = [
    ev(EventKind::Pickup, 0, 0, 0, 0, 15, 3, 0, 0),
    ev(EventKind::Pickup, 1, 0, 0, 9, 20, 4, 0, 0),
    ev(EventKind::Tournament, 3, 1, 12, 17, 25, 0, 40, 0),
    ev(EventKind::Tournament, 2, 1, 24, 45, 30, 0, 42, 0),
    ev(EventKind::Pickup, 4, 2, 24, 0x8000 | 2, 30, 6, 0, 0),
    ev(EventKind::Tournament, 7, 2, 32, 0x8000 | 3, 35, 0, 44, 2),
    ev(EventKind::Pickup, 6, 3, 100, 130, 40, 8, 0, 0),
    ev(EventKind::Tournament, 8, 3, 130, 160, 40, 0, 43, 4),
    ev(EventKind::Tournament, 5, 4, 160, 200, 65, 0, 45, 2),
    ev(EventKind::Tournament, 9, 4, 200, 300, 100, 0, 41, 0),
    // The Rucker Park '78 Legendary Challenge opens at the overall target.
    ev(EventKind::StreetChallenge, 10, 4, 300, 450, 50, 0, 0xff, 6),
    ev(
        EventKind::StreetChallenge,
        1,
        1,
        0,
        0x8000 | 1,
        -1,
        0,
        0x41,
        1,
    ),
    ev(
        EventKind::StreetChallenge,
        3,
        2,
        12,
        0x8000 | 2,
        -1,
        0,
        0x42,
        0,
    ),
    ev(
        EventKind::StreetChallenge,
        2,
        2,
        32,
        0x8000 | 3,
        -1,
        0,
        0x43,
        3,
    ),
    ev(
        EventKind::StreetChallenge,
        7,
        3,
        0x8000 | 5,
        0x8000 | 5,
        -1,
        0,
        0x44,
        2,
    ),
    ev(
        EventKind::StreetChallenge,
        8,
        4,
        130,
        0x8000 | 7,
        -1,
        0,
        0x45,
        5,
    ),
    ev(
        EventKind::StreetChallenge,
        5,
        4,
        200,
        0x8000 | 8,
        -1,
        0,
        0x46,
        4,
    ),
];

/// A street challenge's rules: a row of `0x460970` (16 bytes), picked by
/// its event row's `+0x1b` ([`Event::row`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct StreetRules {
    /// The game type (`FUN_001dbd48`, setup `+0xe8`): the mode's place in
    /// [`crate::Mode::ALL`].
    pub game_type: u32,
    /// The shot clock's row (`FUN_001dbd60`, `+0xec`; [`crate::CLOCKS`]).
    pub clock: u32,
    /// The spot at `FUN_001dbd80`'s index 0 (`+0xf4`: side 1's).
    pub spot: u32,
    /// The target (`FUN_001dbd78`, `+0xf0`).
    pub value: u32,
}

const fn street(game_type: u32, clock: u32, spot: u32, value: u32) -> StreetRules {
    StreetRules {
        game_type,
        clock,
        spot,
        value,
    }
}

/// `0x460970`: Old School to 12 (Soul in the Hole), the Trick Point
/// Challenge to 250,000 (Broad Street), No Gamebreakers to 21 (Greenlake),
/// Dunks Only to 12 on a 12-second clock (Foster Beach), Gamebreaker Wins
/// to a Level 2 (Mosswood; the value is the level less one), League Scoring to
/// 50 (Lincoln College) and Old School to 21 (Rucker Park '78).
pub const STREET_RULES: [StreetRules; 7] = [
    street(5, 2, 0, 12),
    street(1, 2, 0, 250_000),
    street(3, 2, 0, 21),
    street(2, 1, 0, 12),
    street(6, 2, 0, 1),
    street(4, 2, 0, 50),
    street(5, 2, 0, 21),
];

/// The rules an event's game plays (`FUN_001e91a0`): every event first
/// puts the setup's rules back (`FUN_001ded20`: Normal, the shot clock's
/// row 2, to 21, no spots); a street challenge then stores its row of
/// [`STREET_RULES`]. A pick-up and a tournament play the put-back rules: no
/// boss brings rules of his own. The difficulty, the setup's options word
/// (`setup + 300 & 3`), is the player's, as `base` holds it, with its
/// cheats.
pub fn event_rules(ev: usize, base: &crate::Rules) -> crate::Rules {
    let event = &EVENTS[ev];
    let (game_type, clock, spot, value) = match event.kind {
        EventKind::StreetChallenge => {
            let r = STREET_RULES[usize::from(event.row)];
            (r.game_type, r.clock, r.spot, r.value)
        }
        _ => (0, 2, 0, 21),
    };
    crate::Rules::from_setup(game_type, clock, value, [spot, 0], base)
}

/// The map's cities, in the game's order (`mapscreen.txt` 28..33, the
/// city codes 0xa4..0xa9 of the map table).
pub const CITIES: [&str; 6] = [
    "New York",
    "Philly",
    "Chicago",
    "Seattle",
    "Oakland",
    "Los Angeles",
];

/// The map's spots: `{city, event}` in the game's own order (the table at
/// `0x489bd8`, whose rows also carry the two map icons; the map walks it
/// to announce newly opened events, `FUN_002a1c68`).
pub const MAP: [(u8, u8); 17] = [
    (0, 0),
    (0, 12),
    (0, 2),
    (0, 4),
    (0, 6),
    (0, 10),
    (0, 9),
    (1, 11),
    (1, 1),
    (2, 3),
    (2, 13),
    (3, 14),
    (3, 5),
    (4, 8),
    (4, 16),
    (5, 15),
    (5, 7),
];

/// The city an event sits in.
pub fn city_of(ev: usize) -> usize {
    MAP.iter()
        .find(|&&(_, e)| usize::from(e) == ev)
        .map_or(0, |&(c, _)| usize::from(c))
}

/// The events of one city, in the map's order.
pub fn city_events(city: usize) -> Vec<usize> {
    MAP.iter()
        .filter(|&&(c, _)| usize::from(c) == city)
        .map(|&(_, e)| usize::from(e))
        .collect()
}

/// What the map calls an event (`mapscreen.txt` 19..23, chosen as
/// `FUN_002a1c68` does): Pick Up, Tournament (the Rucker Park one is the
/// Street Championship), Street Challenge (the '78 one is the Legendary
/// Challenge). The index is the string's in `mapscreen.txt`.
pub fn kind_word(ev: usize) -> (usize, &'static str) {
    match (EVENTS[ev].kind, ev) {
        (EventKind::Pickup, _) => (19, "Pick Up"),
        (EventKind::Tournament, 9) => (21, "Street Championship"),
        (EventKind::Tournament, _) => (20, "Tournament"),
        (EventKind::StreetChallenge, 10) => (23, "Legendary Challenge"),
        (EventKind::StreetChallenge, _) => (22, "Street Challenge"),
    }
}

/// The street challenges clear bits in this order (`FUN_001eaf90`); the '78
/// challenge holds the final ordinal.
pub fn street_ordinal(ev: usize) -> u32 {
    match ev {
        10 => 6,
        11..=16 => (ev - 11) as u32,
        _ => 0,
    }
}

/// A tournament's opponent: the `{player, team, event}` triples at
/// `0x4609e0`. Beating one admits their star to your crew (the
/// `updateteam.txt` path).
/// Each event's boss move (`0x460790 + 0x1c * row + 0x18`, read by
/// `FUN_001eac00` for tournaments and street challenges; -1 for the
/// pick-ups and the '78 challenge).
pub const BOSS_MOVES: [i16; 17] = [
    -1, -1, 45, 78, -1, 8, -1, 79, 81, 39, -1, 42, 82, 46, 9, 80, 43,
];

/// The Street Legends cards' players (`FUN_001d2818`'s category-0 table
/// at `0x45e150`): card `k` is the `k`th; a tournament's boss is granted
/// his (`FUN_001de528` kind 3 walks the cards for his record).
pub const STREET_CARD_RECORDS: [u16; 9] = [151, 152, 153, 145, 147, 149, 148, 150, 146];

pub const RIVALS: [(u16, u16, u16); 7] = [
    (145, 40, 2),  // Stretch, Team Stretch
    (147, 42, 3),  // Biggie Little, Team Biggie Little
    (149, 44, 5),  // Whitewater
    (148, 43, 7),  // Dime
    (146, 41, 8),  // Bonafide
    (150, 45, 9),  // Osmosis
    (158, 46, 10), // Julius Erving, Rucker Park Legends, at the '78 challenge
];

/// The player record that plays `ev` (0 when the row has no rival).
pub fn rival_of(ev: usize) -> u16 {
    RIVALS
        .iter()
        .find(|&&(_, _, e)| e as usize == ev)
        .map_or(0, |&(p, _, _)| p)
}

/// One reputation level holds this many points (`/0xb` at 0x1d83c8, level
/// 0..5), and every level is worth [`REP_LEVEL_PPS`] overall points
/// (`DAT_00460a78` = {0,10,10,10,10,10}).
pub const REP_LEVEL_SIZE: i16 = 11;
pub const REP_LEVEL_PPS: u32 = 10;
pub const REP_MAX_LEVEL: u8 = 5;

/// Reputation points per game, indexed [event level][reputation level]
/// (`0x460b28`, signed u16s).
#[rustfmt::skip]
const REP_WIN: [[i16; 6]; 5] = [
    [1, 1, 1, 0, 0, 0],
    [2, 1, 1, 1, 0, 0],
    [3, 2, 1, 1, 1, 0],
    [4, 3, 2, 1, 1, 0],
    [5, 4, 3, 2, 1, 0],
];

/// Reputation points lost per game (`0x460b70`).
#[rustfmt::skip]
const REP_LOSS: [[i16; 6]; 5] = [
    [-1, -2, -3, -4, -5, 0],
    [-1, -1, -2, -3, -4, 0],
    [0, -1, -1, -2, -3, 0],
    [0, -1, -1, -2, -3, 0],
    [0, 0, -1, -1, -2, 0],
];

/// Development points by the game's trick points: the 14 rows of
/// `0x460a88` ({u32 threshold, u16 points}), highest first; the game
/// (`FUN_001e9b90`, 0x1ea294) pays the first row the trick points reach.
/// Any game with a point pays 25; the "1,000,000 trick points" milestone
/// sits at the chart's 125 elbow.
const TRICK_DP: [(u32, u32); 14] = [
    (3_000_001, 500),
    (2_750_001, 350),
    (2_500_001, 300),
    (2_250_001, 275),
    (2_000_001, 250),
    (1_750_001, 225),
    (1_500_001, 200),
    (1_250_001, 175),
    (1_000_001, 150),
    (1_000_000, 125),
    (750_000, 100),
    (500_000, 75),
    (250_000, 50),
    (1, 25),
];

/// Development points for a completed court (`FUN_001eaa88(blk, 3, 100)`):
/// a pick-up's target reached, a tournament won, or the Rucker Park '78
/// challenge won, each the first time (`FUN_001e9b90`).
pub const COURT_DP: u32 = 100;

/// The win bonus by difficulty (`FUN_001ea900(blk, 1, won)`: 100, or
/// 150/200 at Mad Game/Legendary while `0x3ec4a0` is set, which the game
/// record's builder does at 0x1daf34); a loss pays none.
const WIN_DP: [u32; 3] = [100, 150, 200];

/// Development points per Level 1 and Level 2 gamebreaker by difficulty:
/// {cap, step} rows at `0x460af8` and `0x460b10`; the game adds a step a
/// gamebreaker until the cap (0x1ea2ec, 0x1ea34c).
const GB1_DP: [(u32, u32); 3] = [(100, 10), (250, 25), (500, 50)];
const GB2_DP: [(u32, u32); 3] = [(225, 25), (450, 50), (900, 100)];

/// The Rucker Park '78 court (XDBCOURT 10), the one street challenge
/// whose win pays the court line.
const RUCKER_78_COURT: u16 = 10;

/// A gamebreaker chart's points for `count` of them.
fn gamebreaker_dp((cap, step): (u32, u32), count: u8) -> u32 {
    (step * u32::from(count)).min(cap)
}

/// Street School's lessons ("lessons complete" counts to 26).
pub const SCHOOL_LESSONS: u32 = 26;

/// The Rewards page's trophies.
pub const TROPHIES: usize = 4;

/// The overall progress target (`FUN_001e74c0`): the ten point events hold
/// 400 between them plus five reputation levels at 10 each. The '78
/// challenge opens at exactly this.
pub const OVERALL_TARGET: u32 = 450;

/// The total the screens' "Progress: %1d/%2d" shows (`FUN_001e74c0`, which
/// Choose Baller reads through `FUN_001eb798` and the map's read-out
/// directly): every event's target (`FUN_001e7550`, the table's -1 counting
/// nothing), 450, and the six reputation entries of `DAT_00460a78` (0 for
/// level 0, then 10 for each of the five levels), 50: 500 in all.
pub fn overall_total() -> u32 {
    let targets: u32 = EVENTS.iter().map(|e| e.target.max(0) as u32).sum();
    targets + REP_LEVEL_PPS * u32::from(REP_MAX_LEVEL)
}

/// The skills, in the database's own field order (`RATING_NAMES`): shots,
/// threes, dunks, rebounds, power, blocks, steals, handles, speed.
pub const SKILLS: [&str; 9] = [
    "Shots", "Threes", "Dunks", "Rebounds", "Power", "Blocks", "Steals", "Handles", "Speed",
];
pub const MAX_RATING: i16 = 99;

/// One unlock or figure the post-game report can show.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Notice {
    /// Reward Points earned (1,000 for the whole League Challenge).
    RewardPoints(u32),
    /// A team is now playable in Pick Up (XDBTEAM id).
    Team(u16),
    /// A court is now playable (XDBCOURT id).
    Court(u16),
    /// A rival player joined the recruit pool (player record).
    Player(u16),
    /// A jersey card granted (`FUN_001de528` kinds 4..6): a street
    /// challenge's (cards 65..70, the event's `+0x1a`), which also opens
    /// its boss's moves.
    Jersey(u16),
    /// A card granted as a completion's (`FUN_001de528` kind 10, one a
    /// game): a Pick Up win threshold's Street Legends card
    /// (0x1df6b0..0x1df780), the fourth trophy's card 34.
    Card(u16),
    /// The Street Champ jersey's line (the career block's result slot 4,
    /// +0xcdc, set by `FUN_001e9b90` at 0x1ea3b4..0x1ea3e8): the Rucker
    /// Park '78 Legendary Challenge (the event on court 10, `FUN_001e8140`)
    /// is cleared and the career has not yet continued past Be A Legend's
    /// completion board (block +0x18 bit 1, set by its Continue at
    /// 0x2621f0).
    StreetChamp,
    /// A boss move newly to be had (`FUN_001de528` kinds 7..9): the
    /// cleared event's `+0x18` move (an `XDBMOVE` row), while its card was
    /// not yet owned.
    BossMove(u8),
    /// Progress points gained in an event (a pick-up's per-game value).
    Progress(u32),
    /// A reputation level was reached.
    Reputation(u8),
    /// A nickname was earned (at reputation level 3, All Star).
    Nickname,
    /// Development points earned (a line of the post-game screen).
    Development(u32),
    /// A tournament goes on: the round (of the bracket's shape) you are
    /// in next.
    NextRound { kind: u8, round: u8 },
    /// Knocked out of a tournament.
    Eliminated,
    /// A game's development points line by line, as the post-game
    /// rewards screen lists them (`bealegendrewards.txt` 1..7, screen
    /// class 35).
    DevBreakdown(DevPoints),
    /// A Street Milestone reached (0-based; the game's "#n" is one more):
    /// its throwback jersey is unlocked.
    Milestone(u8),
    /// An event opened on the map ("Event Unlocked").
    EventOpen(u8),
}

/// A game's development points by source: the story block's five u16
/// lines at +0xce4 (`FUN_001eaa88(blk, line, dp)`), totalled at +0xcd0.
/// The 0.5 a loss writes at +0xcf0 is never read back, so a loss is not
/// halved; it only misses the win bonus.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct DevPoints {
    pub tricks: u32,
    pub level1: u32,
    pub level2: u32,
    pub court: u32,
    pub win_loss: u32,
}

impl DevPoints {
    pub fn total(&self) -> u32 {
        self.tricks + self.level1 + self.level2 + self.court + self.win_loss
    }
}

/// The counters of one match the progression reads (the post-game
/// dispatcher `FUN_001ded40` reads the same handful out of the match).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct MatchOutcome {
    /// Whether the player's team won.
    pub won: bool,
    /// The player's team's trick points.
    pub trick_points: u32,
    /// Level 1 gamebreakers the player's team scored.
    pub level1_gamebreakers: u8,
    /// Level 2 gamebreakers the player's team scored.
    pub level2_gamebreakers: u8,
    /// The difficulty played: 0 Got Game, 1 Mad Game, 2 Legendary.
    pub difficulty: u8,
}

/// Reward Points for a won game, by difficulty (`FUN_001ded40`, which
/// pays the users on the winning side): a Pick Up game 50/100/150, a league
/// Challenge game 100/200/300. Be A Legend pays development points instead.
pub const PICKUP_RP: [u32; 3] = [50, 100, 150];
pub const LEAGUE_RP: [u32; 3] = [100, 200, 300];
/// Finishing the League Challenge (`FUN_001deac8`), by difficulty.
pub const LEAGUE_DONE_RP: [u32; 3] = [1000, 1500, 2000];

/// The whole progression state.
///
/// The League Challenge has five regions. Ordinary team wins use eight bit
/// slots per region in [`Story::league_played`]; the regional legends bosses
/// use independent completion bits in [`Story::region_cleared`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Story {
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    pub development_points: u32,
    /// Development balances by player record. The old shared field holds
    /// only legacy/unassigned points until a baller is selected.
    pub baller_points: Vec<(u16, u32)>,
    pub reward_points: u32,
    /// Eligible Pick Up wins, shared across users/careers (game record +0x1c).
    pub pickup_wins: u32,
    /// Progress points per event, in event order.
    pub progress: [u8; EVENTS.len()],
    /// Reputation points (a level is [`REP_LEVEL_SIZE`]).
    pub reputation: i16,
    /// The player's team (player records): the star first, up to
    /// [`TEAM_SIZE`]; the first three start a match.
    pub crew: Vec<u16>,
    /// Points added to each crew skill by the rewrite's old Gym, which the
    /// original never had (its map's "Edit Baller" opens the player's
    /// editor): bumps an older save bought stay applied; nothing adds to
    /// them now.
    pub upgrades: [i16; SKILLS.len()],
    /// Street challenges cleared, one bit per [`street_ordinal`].
    pub street_cleared: u32,
    /// Tournament events cleared, one bit per event.
    pub tournament_cleared: u32,
    /// Ordinary League Challenge wins, eight bit slots per region.
    pub league_played: u64,
    /// League Challenge's three selected players, independent of a career.
    pub league_roster: Vec<u16>,
    pub league_team: Option<u16>,
    pub league_native_flags: Option<u16>,
    /// Live mode-4 profile identity. None is a guest after selection.
    pub league_user: Option<String>,
    pub league_profile_selected: bool,
    /// Earlier rewrite saves had no owner. Preserve them without attributing
    /// their wins to whichever name happens to enter League Challenge first.
    pub league_unassigned: Option<crate::records::LeagueProgress>,
    /// Regions cleared, one bit each of the five.
    pub region_cleared: u8,
    /// Teams now playable in Pick Up (XDBTEAM records).
    pub unlocked_teams: Vec<u16>,
    /// Players now playable (player records, kept sorted).
    pub unlocked_players: Vec<u16>,
    /// A nickname already earned and its `XDBALIAS` row index (not field-0
    /// id). The bool distinguishes an unset nickname from valid row 0.
    pub nickname: bool,
    pub nickname_id: i16,
    /// Native profile flag +0x19 bit0: an earned alias awaits its call.
    pub nickname_pending: bool,
    /// The career's team, as the crew named it ([`CrewTeam`]).
    pub crew_team: CrewTeam,
    /// The League Challenge trophy earned at least once; run progress is separate.
    pub league_rewarded: bool,
    /// The rewards shop: the card ids bought.
    pub owned: Vec<u16>,
    /// Street School: one bit per lesson completed.
    pub school: u32,
    /// Shared lifetime trophy, separate from the selected user's payload.
    pub school_trophy: bool,
    pub school_user: Option<String>,
    pub school_profile_selected: bool,
    pub school_profile: crate::records::SchoolProgress,
    /// Ownerless earlier rewrite lesson mask and failure summary.
    pub school_unassigned: Option<(u32, bool)>,
    /// Create A Baller: the ballers made so far, each replacing one of the
    /// six create-a-player records.
    pub ballers: Vec<CreatedBaller>,
    /// Create Team: the teams made so far (up to [`TEAM_SLOTS`]).
    pub teams: Vec<CreatedTeam>,
    /// User names, their records and the High Scores.
    pub records: crate::records::Records,
    /// The options, once changed (the defaults until then).
    pub settings: Option<crate::records::Settings>,
    /// Songs the game switched on in [`Story::settings`] by itself (a
    /// court card's, [`Story::court_granted`]) that the live options have
    /// yet to take (not saved).
    pub songs_switched_on: u32,
    /// Tournaments under way (or just finished), by event.
    pub brackets: Vec<(u8, crate::bracket::Bracket)>,
    /// Be A Legend's careers (the game keeps ten, at +0x33f4): the one
    /// being played lives in the fields above; the rest wait here.
    pub careers: [Option<Career>; CAREER_SLOTS],
    /// The career being played, if any.
    pub active: Option<u8>,
    /// Street Milestones reached, one bit each of [`MILESTONES`] (the
    /// game's bits at +0x1c38, `FUN_001e0910`).
    pub milestones: u32,
    /// League Challenge regions with a lost game (milestones 6..10 want none).
    pub league_lost: u8,
    /// A Street School lesson failed once (milestone 12 wants none).
    pub school_failed: bool,
    /// The My Rules modes played in Pick Up, one bit each (milestone 19).
    pub modes_played: u8,
    /// The created ballers who joined a league team after the League Challenge
    /// (Choose Team's "join a team", class 0x4d), as (team, record): the
    /// game data's table at +0x3c, a record or -1 for each of the 29
    /// teams (`FUN_001e0030` sets it, `FUN_001e0000` reads it), which
    /// `FUN_001ebf80(team, 5)` gives as the team's sixth player.
    pub sixth_men: Vec<(u16, u16)>,
}

/// The league run's section tags (its regional boss marker and its
/// roster); the beta's first saves wrote the legacy pair, which is still
/// read.
const LEAGUE_BOSSES: [u8; 4] = *b"LGE2";
const LEAGUE_ROSTER: [u8; 4] = *b"LGE3";
const LEGACY_LEAGUE_BOSSES: [u8; 4] = [0x4e, 0x42, 0x41, 0x32];
const LEGACY_LEAGUE_ROSTER: [u8; 4] = [0x4e, 0x42, 0x41, 0x33];

/// The Street Milestones (`milestone.txt` 3..21); each unlocks the jersey
/// card 46 + its number (`0x460050`).
pub const MILESTONES: usize = 19;
/// The jersey card a milestone unlocks.
pub const MILESTONE_CARD: u16 = 46;

/// Be A Legend careers kept at once ("Choose Baller" lists ten).
pub const CAREER_SLOTS: usize = 10;

/// An event cleared in a career's progress (`FUN_001e7ff8`).
fn event_cleared(
    progress: &[u8; EVENTS.len()],
    tournament_cleared: u32,
    street_cleared: u32,
    ev: usize,
) -> bool {
    match EVENTS[ev].kind {
        EventKind::Pickup => {
            let target = EVENTS[ev].target;
            target >= 0 && i16::from(progress[ev]) >= target
        }
        EventKind::Tournament => tournament_cleared & (1 << ev) != 0,
        EventKind::StreetChallenge => street_cleared & (1 << street_ordinal(ev)) != 0,
    }
}

impl Career {
    /// Event `ev` cleared in this career ([`Story::cleared`]'s rule).
    pub fn cleared(&self, ev: usize) -> bool {
        event_cleared(
            &self.progress,
            self.tournament_cleared,
            self.street_cleared,
            ev,
        )
    }
}

/// One Be A Legend career: its baller (the crew's first) and its own
/// progress. The rest of the save (rewards, Street School, the league
/// Challenge, created ballers and teams, records) is shared.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Career {
    pub games: u32,
    pub wins: u32,
    pub losses: u32,
    pub progress: [u8; EVENTS.len()],
    pub reputation: i16,
    pub crew: Vec<u16>,
    pub upgrades: [i16; SKILLS.len()],
    pub street_cleared: u32,
    pub tournament_cleared: u32,
    pub nickname: bool,
    pub nickname_id: i16,
    /// Native profile flag +0x19 bit0: an earned alias awaits its call.
    pub nickname_pending: bool,
    pub brackets: Vec<(u8, crate::bracket::Bracket)>,
    /// The crew's team as its keyboards and Choose Logo named it.
    pub team: CrewTeam,
}

/// A career's team as the crew's Create Team names it (`FUN_00280cd0`:
/// the city's keyboard, the name's and Choose Logo, `FUN_001edc50`,
/// `FUN_001edc88`, `FUN_001edd20` on the career's own team record at
/// `+0x664`); empty for a career from before the rewrite kept it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CrewTeam {
    pub city: String,
    pub name: String,
    /// An `XATEAMS` image (the created teams' 29..38).
    pub logo: u8,
}

impl Career {
    /// The calendar week: native Be A Legend maps games to weeks as `games / 6 + 1`.
    pub fn week(&self) -> u32 {
        self.games / 6 + 1
    }

    fn encode(&self, out: &mut Vec<u8>) {
        for v in [self.games, self.wins, self.losses] {
            out.extend_from_slice(&v.to_le_bytes());
        }
        out.extend_from_slice(&self.progress);
        out.extend_from_slice(&self.reputation.to_le_bytes());
        out.push(self.crew.len().min(TEAM_SIZE) as u8);
        for r in self.crew.iter().take(TEAM_SIZE) {
            out.extend_from_slice(&r.to_le_bytes());
        }
        for u in self.upgrades {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&self.street_cleared.to_le_bytes());
        out.extend_from_slice(&self.tournament_cleared.to_le_bytes());
        out.push(u8::from(self.nickname));
        out.extend_from_slice(&self.nickname_id.to_le_bytes());
        out.push(self.brackets.len() as u8);
        for (ev, b) in &self.brackets {
            out.push(*ev);
            b.encode(out);
        }
    }

    fn decode(b: &[u8]) -> Option<(Self, usize)> {
        fn take<'a>(rest: &mut &'a [u8], n: usize) -> Option<&'a [u8]> {
            let out = rest.get(..n)?;
            *rest = &rest[n..];
            Some(out)
        }
        let u32_ = |s: &[u8]| u32::from_le_bytes(s.try_into().unwrap());
        let i16_ = |s: &[u8]| i16::from_le_bytes(s.try_into().unwrap());
        let mut rest = b;
        let r = &mut rest;
        let mut c = Career {
            games: u32_(take(r, 4)?),
            wins: u32_(take(r, 4)?),
            losses: u32_(take(r, 4)?),
            ..Default::default()
        };
        c.progress.copy_from_slice(take(r, EVENTS.len())?);
        c.reputation = i16_(take(r, 2)?);
        let n = usize::from(take(r, 1)?[0]);
        if n > TEAM_SIZE {
            return None;
        }
        for _ in 0..n {
            c.crew
                .push(u16::from_le_bytes(take(r, 2)?.try_into().unwrap()));
        }
        for u in &mut c.upgrades {
            *u = i16_(take(r, 2)?);
        }
        c.street_cleared = u32_(take(r, 4)?);
        c.tournament_cleared = u32_(take(r, 4)?);
        c.nickname = take(r, 1)?[0] != 0;
        c.nickname_id = i16_(take(r, 2)?);
        let brackets = usize::from(take(r, 1)?[0]);
        for _ in 0..brackets {
            let ev = take(r, 1)?[0];
            if usize::from(ev) >= EVENTS.len() {
                return None;
            }
            let (bracket, used) = crate::bracket::Bracket::decode(r)?;
            take(r, used)?;
            c.brackets.push((ev, bracket));
        }
        Some((c, b.len() - rest.len()))
    }
}

/// Be A Legend's team: your star and four more (the roster at story
/// +0x6dc, five u16s).
pub const TEAM_SIZE: usize = 5;

/// How many teams can be made ("There are already 10 existing Created
/// Teams.").
pub const TEAM_SLOTS: usize = 10;
/// Players on a created team ("Select 5 players for your team.").
pub const TEAM_PLAYERS: usize = 5;

/// A team made in Create Team: where it's from, its name, its logo (an
/// `XATEAMS` image) and five player records.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreatedTeam {
    pub city: String,
    pub name: String,
    pub logo: u8,
    pub roster: [u16; TEAM_PLAYERS],
}

/// The first of the six create-a-player records (team 37: 234..239).
pub const BALLER_RECORDS: u16 = 234;
/// The league's teams, the database's first (`FUN_0026e558`: 0x1d).
pub const LEAGUE_TEAMS: usize = 29;
/// How many ballers can be made (one per create-a-player record).
pub const BALLER_SLOTS: usize = 6;
/// Every baller a save can hold: the six Create A Baller slots, then career
/// stars past them. The original keeps each Be A Legend career's player
/// inside the career (+0x6d4, ids 245 + career), so an imported save with
/// more careers than slots needs its own record per extra star
/// (`BALLER_RECORDS + slot`, past the disc's 240 records).
pub const STAR_SLOTS: usize = 10;

/// A baller made in Create A Baller: the appearance and skills that replace
/// create-a-player record `BALLER_RECORDS + slot`. The fields are the
/// database's (see `street_assets::db::Player`); the game clamps the
/// appearance numbers to what the kit holds for the build.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CreatedBaller {
    pub slot: u8,
    pub first_name: String,
    pub last_name: String,
    pub female: bool,
    /// 0..3 men (slender to big), 4..5 women.
    pub build: u8,
    pub height_in: u8,
    pub weight_lb: u16,
    /// The face.
    pub look: u8,
    /// -1 for none (bald).
    pub hair_style: i8,
    pub hair_color: u8,
    /// -1 for none.
    pub facial_hair_style: i8,
    pub facial_hair_color: u8,
    /// Outfit 0..10 and its colour variant 0..2.
    pub clothing: u8,
    pub clothing_variant: u8,
    pub shoes: u8,
    /// The front end's seven skills (shots, dunks, power, blocks, steals,
    /// handles, rebounds), 0..99.
    pub skills: [u8; 7],
    /// Equipped native slots as `XDBMOVE` rows, with [`EMPTY_MOVE_SLOT`] holes:
    /// lay-ups, dunks and dribble moves, at most [`MOVE_SLOTS`] of each (the record's
    /// fields 68..73, 79..89 and 90..100, which 0x1d8778 reads).
    pub moves: [Vec<u8>; 3],
    /// Every move bought, assigned or not: a move replaced in its slot can
    /// come back for free ("Old moves that are replaced can be restored
    /// for free at a later date", `editplyr_moves_replace.txt`).
    pub unlocked: Vec<u8>,
    /// The outfits and shoes bought (the career's +0x670..+0x678).
    pub wardrobe: crate::cab::Wardrobe,
    /// The baller's icon, 0..6: the record's field 14, which the portrait
    /// tiles show as sheet 0xb0 + it (`FUN_00223d88`); picked on Create
    /// Baller's name panel ("Select Baller Icon").
    pub icon: u8,
}

/// How many lay-ups, dunks and dribble moves a created player holds.
pub const MOVE_SLOTS: [usize; 3] = [6, 11, 11];

/// A vacant native equipped slot (the original stores -1).
pub const EMPTY_MOVE_SLOT: u8 = u8::MAX;

impl CreatedBaller {
    /// The outfit's row in the build's [`crate::cab::OUTFITS`] (row 0 for a
    /// clothing id the build does not list).
    pub fn outfit_row(&self) -> usize {
        crate::cab::OUTFITS[usize::from(self.build.min(5))]
            .iter()
            .position(|&id| id == self.clothing)
            .unwrap_or(0)
    }

    /// A wardrobe owning just what the baller wears (a special outfit,
    /// code 0x36 and up, is no row of it).
    pub fn worn_wardrobe(&self) -> crate::cab::Wardrobe {
        let mut w = crate::cab::Wardrobe::default();
        if self.clothing < crate::cab::SPECIAL_OUTFIT {
            w.buy_outfit(
                self.outfit_row(),
                usize::from(self.clothing_variant.min(2)),
                false,
            );
        }
        w.buy_shoes(crate::cab::shoe_row(self.shoes));
        w
    }

    /// The player record this baller replaces.
    pub fn record(&self) -> usize {
        usize::from(BALLER_RECORDS) + usize::from(self.slot)
    }

    /// A career's own star, kept past the six Create A Baller slots.
    pub fn career_only(&self) -> bool {
        usize::from(self.slot) >= BALLER_SLOTS
    }

    /// Whether a move (an `XDBMOVE` row) is the baller's to assign: bought
    /// once, or in a slot now.
    pub fn owns(&self, row: u8) -> bool {
        self.unlocked.contains(&row) || self.moves.iter().any(|m| m.contains(&row))
    }

    /// Assign a row to a particular native slot, preserving vacant positions.
    pub fn assign_in_slot(&mut self, kind: usize, slot: usize, row: u8) -> bool {
        let Some(list) = self.moves.get_mut(kind) else {
            return false;
        };
        if slot >= MOVE_SLOTS[kind] || row == EMPTY_MOVE_SLOT || list.contains(&row) {
            return false;
        }
        list.resize(MOVE_SLOTS[kind], EMPTY_MOVE_SLOT);
        list[slot] = row;
        if !self.unlocked.contains(&row) {
            self.unlocked.push(row);
        }
        true
    }

    /// Edit Moves: `row` takes the place of `old` in the `kind` slots, or
    /// an open slot when `old` is `None`; the move is unlocked for good.
    /// Returns whether it went in (a full list or a move already in a slot
    /// refuses).
    pub fn assign(&mut self, kind: usize, old: Option<u8>, row: u8) -> bool {
        let Some(list) = self.moves.get_mut(kind) else {
            return false;
        };
        if list.contains(&row) {
            return false;
        }
        match old.and_then(|o| list.iter().position(|&m| m == o)) {
            Some(at) => list[at] = row,
            None if list.len() < MOVE_SLOTS[kind] => list.push(row),
            None => return false,
        }
        if !self.unlocked.contains(&row) {
            self.unlocked.push(row);
        }
        true
    }
}

/// An League Challenge region's matches, from the table at `0x3ec9e8`: the
/// opponents are league team ids. Each region then plays its legends team.
pub const REGION_MATCHES: [&[u16]; 5] = [
    &[7, 22, 23, 25, 27, 5],
    &[2, 3, 6, 9, 14, 15],
    &[4, 8, 10, 11, 21, 24],
    &[16, 1, 28, 18, 26, 20],
    &[19, 13, 12, 17, 0],
];

/// The regions' names, as `choosergn.txt` lists them.
pub const REGION_NAMES: [&str; 5] = [
    "North West",
    "Central",
    "South West",
    "North East",
    "South East",
];

/// What finishing a region pays: the region's legends team and its court
/// (`FUN_001de528(rec, 2, court)` and `FUN_001de528(rec, 1, team)`). The
/// boss is the extra opponent at index `REGION_MATCHES[region].len()`.
pub const REGION_REWARDS: [(u16, u16); 5] = [(31, 7), (32, 2), (33, 8), (34, 4), (35, 1)];

const fn league_bit(region: usize, index: usize) -> u64 {
    1u64 << (region * 8 + index)
}

impl Story {
    /// A baller's own points; an unassigned legacy balance can be claimed
    /// once by the first baller who receives or spends points.
    pub fn points_for(&self, record: u16) -> u32 {
        self.baller_points
            .iter()
            .find(|(r, _)| *r == record)
            .map_or(0, |(_, n)| *n)
            .saturating_add(self.development_points)
    }

    pub fn star_points(&self) -> u32 {
        self.crew
            .first()
            .map_or(self.development_points, |&r| self.points_for(r))
    }

    pub fn school_progress(&self) -> crate::records::SchoolProgress {
        let mut progress = self.school_profile.clone();
        for (lesson, value) in progress.challenges[..SCHOOL_LESSONS as usize]
            .iter_mut()
            .enumerate()
        {
            *value = (*value & 7) | (u8::from(self.school & (1 << lesson) != 0) << 3);
        }
        progress
    }

    pub fn select_school_user(&mut self, user: Option<usize>) -> bool {
        if user.is_some_and(|index| index >= self.records.users.len()) {
            return false;
        }
        self.sync_school_user();
        if !self.school_profile_selected {
            self.school_unassigned = Some((self.school, self.school_failed));
            self.school_trophy |=
                self.school & ((1 << SCHOOL_LESSONS) - 1) == (1 << SCHOOL_LESSONS) - 1;
        }
        let (name, progress) = user.map_or_else(
            || (None, crate::records::SchoolProgress::default()),
            |index| {
                let user = &self.records.users[index];
                (Some(user.name.clone()), user.school.clone())
            },
        );
        self.school_user = name;
        self.school_profile_selected = true;
        self.school = progress.completed();
        self.school_failed = progress.failed();
        self.school_profile = progress;
        true
    }

    pub fn sync_school_user(&mut self) {
        let Some(index) = self
            .school_user
            .as_deref()
            .and_then(|name| self.records.user(name))
        else {
            return;
        };
        self.records.users[index].school = self.school_progress();
    }

    /// FUN_001eeca8 runs at launch, including a replay of a completed lesson.
    pub fn start_school_lesson(&mut self, lesson: usize) -> bool {
        let Some(attempts) = self.school_profile.attempts.get_mut(lesson) else {
            return false;
        };
        *attempts = attempts.wrapping_add(1);
        self.sync_school_user();
        true
    }

    /// FUN_001eecd8: success sets bit3, failure sets low bits to4; a
    /// completed lesson's nibble is never rewritten by a later replay.
    pub fn finish_school_lesson(&mut self, lesson: usize, passed: bool) -> bool {
        if lesson >= SCHOOL_LESSONS as usize {
            return false;
        }
        let value = &mut self.school_profile.challenges[lesson];
        if *value & 8 == 0 {
            if passed {
                *value |= 8;
            } else {
                *value = (*value & 8) | 4;
            }
        }
        self.school = self.school_profile.completed();
        self.school_failed = self.school_profile.failed();
        self.sync_school_user();
        true
    }

    /// Mode1's dispatcher pays1000 shared RP once for each live user
    /// payload. Native user+0x54 bit0 prevents repeated payouts.
    pub fn school_completion_rewards(&mut self) -> Vec<Notice> {
        let all = (1 << SCHOOL_LESSONS) - 1;
        if !self.school_profile_selected
            || !self.school_profile.payout_known
            || self.school & all != all
            || self.school_profile.flags & 1 != 0
        {
            return Vec::new();
        }
        self.reward_points += 1000;
        let mut notes = vec![Notice::RewardPoints(1000)];
        if self.reach_milestone(10) {
            notes.push(Notice::Milestone(10));
        }
        if !self.school_profile.failed() && self.reach_milestone(11) {
            notes.push(Notice::Milestone(11));
        }
        self.school_trophy = true;
        self.school_profile.flags |= 1;
        self.sync_school_user();
        notes
    }

    pub fn league_progress(&self) -> crate::records::LeagueProgress {
        crate::records::LeagueProgress {
            played: self.league_played,
            cleared: self.region_cleared,
            lost: self.league_lost,
            team: self.league_team,
            native_flags: self.league_native_flags,
            roster: self.league_roster.clone(),
        }
    }

    /// Bind the selected name's own Challenge payload. A guest gets a live
    /// payload but never a stored User entry (native FUN_001ee450).
    pub fn select_league_user(&mut self, user: Option<usize>) -> bool {
        if user.is_some_and(|index| index >= self.records.users.len()) {
            return false;
        }
        self.sync_league_user();
        if !self.league_profile_selected {
            self.league_unassigned = Some(self.league_progress());
        }
        let (name, progress) = user.map_or_else(
            || (None, crate::records::LeagueProgress::default()),
            |index| {
                let user = &self.records.users[index];
                (Some(user.name.clone()), user.league.clone())
            },
        );
        self.league_user = name;
        self.league_profile_selected = true;
        self.league_played = progress.played;
        self.region_cleared = progress.cleared;
        self.league_lost = progress.lost;
        self.league_team = progress.team;
        self.league_native_flags = progress.native_flags;
        self.league_roster = progress.roster;
        true
    }

    pub fn sync_league_user(&mut self) {
        let Some(index) = self
            .league_user
            .as_deref()
            .and_then(|name| self.records.user(name))
        else {
            return;
        };
        self.records.users[index].league = self.league_progress();
    }

    /// FUN_001ee3e0 starts a clean region only when no region is pending.
    pub fn start_league_region(&mut self, region: usize) {
        if region >= REGION_MATCHES.len() {
            return;
        }
        let flags = self.league_native_flags.unwrap_or(0x3f);
        if flags & 0x80 == 0 {
            self.league_native_flags = Some((flags & !0x3f) | region as u16 | 0x40);
            self.league_lost &= !(1 << region);
            self.sync_league_user();
        }
    }

    pub fn select_league_roster(&mut self, roster: &[u16]) -> bool {
        if roster.len() != 3
            || roster[0] == roster[1]
            || roster[0] == roster[2]
            || roster[1] == roster[2]
        {
            return false;
        }
        self.league_roster = roster.to_vec();
        self.sync_league_user();
        true
    }

    pub fn credit_baller(&mut self, record: u16, points: u32) {
        let legacy = std::mem::take(&mut self.development_points);
        if let Some((_, balance)) = self.baller_points.iter_mut().find(|(r, _)| *r == record) {
            *balance = balance.saturating_add(legacy).saturating_add(points);
        } else {
            self.baller_points
                .push((record, legacy.saturating_add(points)));
            self.baller_points.sort_by_key(|(r, _)| *r);
        }
    }

    pub fn spend_baller(&mut self, record: u16, points: u32) -> bool {
        if self.points_for(record) < points {
            return false;
        }
        self.credit_baller(record, 0);
        self.baller_points
            .iter_mut()
            .find(|(r, _)| *r == record)
            .unwrap()
            .1 -= points;
        true
    }

    pub fn credit_star(&mut self, points: u32) {
        if let Some(&record) = self.crew.first() {
            self.credit_baller(record, points);
        } else {
            self.development_points = self.development_points.saturating_add(points);
        }
    }

    /// Development cards select their receiving baller before charging RP.
    pub fn spend_reward_points_for(&mut self, record: u16, rp: u32, dp: u32) -> bool {
        if self.reward_points < rp {
            return false;
        }
        self.reward_points -= rp;
        self.credit_baller(record, dp);
        true
    }

    pub fn reputation_level(&self) -> u8 {
        (self.reputation / REP_LEVEL_SIZE).clamp(0, i16::from(REP_MAX_LEVEL)) as u8
    }

    /// Overall progress points: every event's held progress plus ten per
    /// reputation level (`FUN_001e73d8`).
    pub fn overall_points(&self) -> u32 {
        self.progress.iter().fold(0u32, |n, &p| n + u32::from(p))
            + u32::from(self.reputation_level()) * REP_LEVEL_PPS
    }

    /// A tournament's bracket: the one under way, else a fresh one (the
    /// boss crew in it until the tournament has been won once). The
    /// fresh one's crews are numbered from the games played, so it is the
    /// same until a game is played.
    pub fn tournament(&self, ev: usize) -> crate::bracket::Bracket {
        if let Some((_, b)) = self.brackets.iter().find(|(e, _)| usize::from(*e) == ev)
            && !b.won()
            && !b.out()
        {
            return b.clone();
        }
        let row = EVENTS[ev];
        let kind = match row.format {
            Format::SingleElimination => 0,
            Format::DoubleElimination => 2,
            Format::BestOfThree => 4,
        };
        let boss = (row.boss_team != 0 && row.boss_team != 0xff && !self.cleared(ev))
            .then_some(row.boss_team);
        let seed = (ev as u32) << 20 ^ self.games.wrapping_mul(2_654_435_761);
        crate::bracket::Bracket::new(kind, boss, seed)
    }

    /// Whether Update Team follows a story game (the end board's case 5,
    /// `FUN_0029d5a8`, and the bracket screens): a won pick-up or street
    /// challenge where one of the beaten players isn't on your team yet,
    /// or a tournament whose title this game won (the bracket screen's
    /// check of the final's winner, `FUN_001e8580` = your crew 0x25).
    pub fn offers_update(&self, ev: usize, won: bool, beaten: &[u16]) -> bool {
        if !won || ev >= EVENTS.len() {
            return false;
        }
        match EVENTS[ev].kind {
            EventKind::Tournament => self
                .brackets
                .iter()
                .any(|(e, b)| usize::from(*e) == ev && b.won()),
            _ => beaten.iter().any(|p| !self.crew.contains(p)),
        }
    }

    /// Update Team's roster change (`FUN_001e7db8`): `player` takes roster
    /// position `slot` (1..=4; your star at 0 never moves). A position past
    /// the end of the team fills the next open one. A player already on the
    /// team changes nothing. Returns whether the team changed.
    pub fn update_team(&mut self, slot: usize, player: u16) -> bool {
        if !(1..TEAM_SIZE).contains(&slot) || self.crew.is_empty() || self.crew.contains(&player) {
            return false;
        }
        match self.crew.get_mut(slot) {
            Some(at) => *at = player,
            None => self.crew.push(player),
        }
        true
    }

    /// Puts a team member among the three starters, or takes them out of
    /// it: the star stays first; a bench player swaps with the last starter.
    pub fn toggle_starter(&mut self, member: usize) {
        if member == 0 || member >= self.crew.len() {
            return;
        }
        if member >= 3 {
            self.crew.swap(2, member);
        } else if self.crew.len() > 3 {
            // A starter goes to the bench: the first bench player comes in.
            let out = self.crew.remove(member);
            self.crew.push(out);
        }
    }

    /// The career being played, as it stands.
    pub fn career(&self) -> Career {
        Career {
            games: self.games,
            wins: self.wins,
            losses: self.losses,
            progress: self.progress,
            reputation: self.reputation,
            crew: self.crew.clone(),
            upgrades: self.upgrades,
            street_cleared: self.street_cleared,
            tournament_cleared: self.tournament_cleared,
            nickname: self.nickname,
            nickname_id: self.nickname_id,
            nickname_pending: self.nickname_pending,
            brackets: self.brackets.clone(),
            team: self.crew_team.clone(),
        }
    }

    fn set_career(&mut self, c: Career) {
        self.games = c.games;
        self.wins = c.wins;
        self.losses = c.losses;
        self.progress = c.progress;
        self.reputation = c.reputation;
        self.crew = c.crew;
        self.upgrades = c.upgrades;
        self.street_cleared = c.street_cleared;
        self.tournament_cleared = c.tournament_cleared;
        self.nickname = c.nickname;
        self.nickname_id = c.nickname_id;
        self.nickname_pending = c.nickname_pending;
        self.brackets = c.brackets;
        self.crew_team = c.team;
    }

    /// Career `slot` as it stands (the one being played from the fields).
    pub fn career_at(&self, slot: usize) -> Option<Career> {
        if self.active == Some(slot as u8) {
            Some(self.career())
        } else {
            self.careers.get(slot)?.clone()
        }
    }

    /// Plays career `slot` from now on ("Continue Career"): the one being
    /// played is put away first.
    pub fn switch_career(&mut self, slot: usize) -> bool {
        if slot >= CAREER_SLOTS || self.careers[slot].is_none() {
            return false;
        }
        if let Some(a) = self.active {
            self.careers[usize::from(a)] = Some(self.career());
        }
        let c = self.careers[slot].clone().unwrap_or_default();
        self.set_career(c);
        self.active = Some(slot as u8);
        true
    }

    /// A new career in `slot` with `star` (a create-a-player record) as
    /// its baller.
    pub fn start_career(&mut self, slot: usize, star: u16) -> bool {
        if slot >= CAREER_SLOTS {
            return false;
        }
        if let Some(a) = self.active {
            self.careers[usize::from(a)] = Some(self.career());
        }
        let c = Career {
            crew: vec![star],
            ..Default::default()
        };
        self.careers[slot] = Some(c.clone());
        self.set_career(c);
        self.active = Some(slot as u8);
        true
    }

    /// How far career `slot` has come: its overall progress points, and
    /// whether every event is cleared.
    pub fn career_standing(&self, slot: usize) -> Option<(u32, bool)> {
        let c = self.career_at(slot)?;
        let mut s = Story::default();
        s.set_career(c);
        Some((s.overall_points(), s.legend_complete()))
    }

    /// Ends career `slot` for good.
    pub fn delete_career(&mut self, slot: usize) {
        if slot >= CAREER_SLOTS {
            return;
        }
        self.careers[slot] = None;
        if self.active == Some(slot as u8) {
            self.active = None;
            self.set_career(Career::default());
        }
    }

    /// Gives up a tournament under way.
    pub fn forfeit(&mut self, ev: usize) {
        self.brackets.retain(|(e, _)| usize::from(*e) != ev);
    }

    /// The calendar week: native Be A Legend maps games to weeks as `games / 6 + 1`.
    pub fn week(&self) -> u32 {
        self.games / 6 + 1
    }

    /// Be A Legend finished: every event cleared.
    pub fn legend_complete(&self) -> bool {
        (0..EVENTS.len()).all(|ev| self.cleared(ev))
    }

    /// An event cleared: pick-ups by their target, tournaments and street
    /// challenges by their own cleared bits.
    pub fn cleared(&self, ev: usize) -> bool {
        event_cleared(
            &self.progress,
            self.tournament_cleared,
            self.street_cleared,
            ev,
        )
    }

    fn gate(&self, g: Gate) -> bool {
        match g {
            Gate::Points(n) => self.overall_points() >= n,
            Gate::Clear(ev) => self.cleared(ev as usize),
            Gate::Now => true,
        }
    }

    /// Whether the event appears on the map at all.
    pub fn visible(&self, ev: usize) -> bool {
        self.gate(EVENTS[ev].visible)
    }

    /// Whether the event can start.
    pub fn open(&self, ev: usize) -> bool {
        self.gate(EVENTS[ev].open)
    }

    /// The charts' lines for one game (`FUN_001e9b90`): trick points,
    /// Level 1 and Level 2 gamebreakers by difficulty, and the win bonus;
    /// the court line is the caller's.
    fn chart_lines(out: &MatchOutcome) -> DevPoints {
        let d = usize::from(out.difficulty.min(2));
        DevPoints {
            tricks: TRICK_DP
                .iter()
                .find(|&&(t, _)| out.trick_points >= t)
                .map_or(0, |&(_, dp)| dp),
            level1: gamebreaker_dp(GB1_DP[d], out.level1_gamebreakers),
            level2: gamebreaker_dp(GB2_DP[d], out.level2_gamebreakers),
            court: 0,
            win_loss: if out.won { WIN_DP[d] } else { 0 },
        }
    }

    /// Play one story event game. Returns the notices the post-game screen
    /// shows (in the game's own order: gains first, then unlocks).
    pub fn play(&mut self, ev: usize, out: &MatchOutcome) -> Vec<Notice> {
        let row = EVENTS[ev];
        let was_open: Vec<bool> = (0..EVENTS.len()).map(|e| self.open(e)).collect();
        self.wins += u32::from(out.won);
        self.losses += u32::from(!out.won);
        let mut notes = Vec::new();

        // The DP charts (`FUN_001e9b90`).
        let mut lines = Self::chart_lines(out);
        let was_cleared = self.cleared(ev);

        // Progress: a won pick-up pays its per-game value clamped by the
        // target left (the pick-up branch of `FUN_001e9b90` only on a
        // win); tournaments and street challenges pay their target on the
        // win that clears them (`likely` — their per-game column carries a
        // poster id instead, so the game grants their progress on clearing).
        let mut gained = 0u8;
        match row.kind {
            EventKind::Pickup => {
                self.games += 2;
                let room = (i32::from(row.target) - i32::from(self.progress[ev])).max(0) as u8;
                if out.won {
                    gained = row.points_per_game.min(room);
                }
                // Reaching the target completes the court.
                if gained > 0
                    && !was_cleared
                    && i16::from(self.progress[ev]) + i16::from(gained) >= row.target
                {
                    lines.court = COURT_DP;
                }
            }
            EventKind::Tournament => {
                // The game goes in the bracket; the title clears it.
                let mut b = self.tournament(ev);
                b.play(out.won);
                let champion = b.won();
                if b.out() {
                    self.games += 6;
                    notes.push(Notice::Eliminated);
                } else if let Some((round, _, _)) = b.current() {
                    notes.push(Notice::NextRound {
                        kind: b.kind,
                        round: round as u8,
                    });
                }
                self.brackets.retain(|(e, _)| usize::from(*e) != ev);
                self.brackets.push((ev as u8, b));
                if champion {
                    self.games += 6;
                    if !self.cleared(ev) {
                        gained = row.target.clamp(0, i16::from(u8::MAX)) as u8;
                        lines.court = COURT_DP;
                        self.clear_event(ev, &mut notes);
                    }
                }
            }
            EventKind::StreetChallenge => {
                self.games += 2;
                if out.won && !self.cleared(ev) {
                    gained = row.target.clamp(0, i16::from(u8::MAX)) as u8;
                    // Only the Rucker Park '78 challenge (court 10) pays
                    // the court line (`FUN_001eab50(ev) == 10`).
                    if row.court == RUCKER_78_COURT {
                        lines.court = COURT_DP;
                    }
                    self.clear_event(ev, &mut notes);
                }
            }
        }
        let dp = lines.total();
        self.progress[ev] = self.progress[ev].saturating_add(gained);
        if gained > 0 {
            notes.insert(0, Notice::Progress(u32::from(gained)));
        }
        if dp > 0 {
            self.credit_star(dp);
            notes.insert(0, Notice::Development(dp));
            notes.insert(0, Notice::DevBreakdown(lines));
        }

        // Reputation: the game's tables, a level's band of movement a game.
        let before = (self.reputation, self.reputation_level());
        let columns = if row.level as usize >= REP_WIN.len() {
            0i16
        } else {
            let table = if out.won { &REP_WIN } else { &REP_LOSS };
            table[row.level as usize][usize::from(before.1).min(5)]
        };
        // FUN_001e70e0: losses cannot demote; wins advance at most one
        // tier, discarding overflow. Tier 5 is terminal.
        if before.1 < REP_MAX_LEVEL {
            let base = i16::from(before.1) * REP_LEVEL_SIZE;
            let progress = (self.reputation - base).max(0);
            self.reputation = base
                + (i32::from(progress) + i32::from(columns)).clamp(0, i32::from(REP_LEVEL_SIZE))
                    as i16;
        }
        let after_level = self.reputation_level();
        if after_level > before.1 {
            notes.push(Notice::Reputation(after_level));
            for level in before.1 + 1..=after_level {
                let n = match level {
                    1 => Some(14),
                    2 => Some(15),
                    4 => Some(16),
                    5 => Some(17),
                    _ => None,
                };
                if let Some(n) = n
                    && self.reach_milestone(n)
                {
                    notes.push(Notice::Milestone(n));
                }
            }
        } else if columns != 0 && self.reputation != before.0 {
            notes.push(Notice::Reputation(after_level));
        }
        if after_level == 3 && !self.nickname {
            self.nickname = true;
            self.nickname_pending = true;
            notes.push(Notice::Nickname);
        }
        // What the game opened: the map announces them in its own order.
        for &(_, e) in &MAP {
            let e = usize::from(e);
            if !was_open[e] && self.open(e) && self.visible(e) {
                notes.push(Notice::EventOpen(e as u8));
            }
        }
        notes
    }

    /// What clearing an event grants, except the DP and progress the
    /// caller adds.
    fn clear_event(&mut self, ev: usize, notes: &mut Vec<Notice>) {
        match EVENTS[ev].kind {
            EventKind::Tournament => self.tournament_cleared |= 1 << ev,
            EventKind::StreetChallenge => self.street_cleared |= 1 << street_ordinal(ev),
            EventKind::Pickup => return,
        }
        // `FUN_001ded40` on the event's completion (0x1deed4..0x1df000),
        // after its court: the boss move while its card is not owned (kind
        // 7), a street challenge's jersey card (kind 4), a tournament's
        // boss and his card (kind 3, the card whose player he is).
        let rival = rival_of(ev);
        let card = match EVENTS[ev].kind {
            EventKind::StreetChallenge => Some(EVENTS[ev].boss_team),
            _ => STREET_CARD_RECORDS
                .iter()
                .position(|&r| r == rival)
                .map(|k| k as u16),
        };
        if let (Ok(row), Some(card)) = (u8::try_from(BOSS_MOVES[ev]), card)
            && !self.owned.contains(&card)
        {
            notes.push(Notice::BossMove(row));
        }
        if EVENTS[ev].kind == EventKind::StreetChallenge
            && let Some(card) = card.filter(|c| (0x41..=0x46).contains(c))
            && !self.owned.contains(&card)
        {
            self.owned.push(card);
            notes.push(Notice::Jersey(card));
        }
        // The rival (tournament) and, for the '78 challenge, his team come
        // from the triples at 0x4609e0.
        if let Some(&(player, team, _)) = RIVALS.iter().find(|&&(_, _, e)| e as usize == ev) {
            if player != 0 && !self.unlocked_players.contains(&player) {
                self.unlocked_players.push(player);
                self.unlocked_players.sort_unstable();
                notes.push(Notice::Player(player));
                if EVENTS[ev].kind == EventKind::Tournament
                    && let Some(card) = card
                    && !self.owned.contains(&card)
                {
                    self.owned.push(card);
                }
            }
            if team >= 29 && !self.unlocked_teams.contains(&team) {
                self.unlocked_teams.push(team);
                self.unlocked_teams.sort_unstable();
                notes.push(Notice::Team(team));
            }
        }
    }

    /// One League Challenge match. The regional legends become available
    /// after all ordinary teams, and their win sets the native +0x50 bit.
    pub fn play_league(&mut self, region: usize, index: usize, out: &MatchOutcome) -> Vec<Notice> {
        let Some(teams) = REGION_MATCHES.get(region) else {
            return Vec::new();
        };
        let boss = index == teams.len();
        if index > teams.len() || (boss && !self.league_boss_open(region)) {
            return Vec::new();
        }
        let was_complete = self.complete_league();
        // FUN_001ee790 commits this region's own league team after either
        // ordinary result; FUN_001ee9a0 does so only for a boss win. The
        // automatic predicate alone would forget it when teams change.
        if (!boss || out.won)
            && let Some(team) = self.league_team.filter(|&team| team < 29)
            && let Some(own_index) = teams.iter().position(|&opponent| opponent == team)
        {
            self.league_played |= league_bit(region, own_index);
        }
        // Mode 4 updates user aggregates through FUN_001ee420, never the
        // active Be A Legend career's FUN_001e70a8 counters. The game layer
        // records this match for its named users independently.
        // Only a win beats the team; a loss spoils the region's clean run.
        if out.won && !boss {
            self.league_played |= league_bit(region, index);
        } else if !out.won {
            self.league_lost |= 1 << region;
        }
        let mut notes = Vec::new();
        let difficulty = usize::from(out.difficulty.min(2));
        if out.won {
            let rp = LEAGUE_RP[difficulty];
            self.reward_points += rp;
            notes.push(Notice::RewardPoints(rp));
        }

        if out.won && boss && self.region_cleared & (1 << region) == 0 {
            self.region_cleared |= 1 << region;
            // Milestones 6..10: a region without a loss (`FUN_001ded40`).
            if self.league_lost & (1 << region) == 0 {
                let n = 5 + region as u8;
                if self.reach_milestone(n) {
                    notes.push(Notice::Milestone(n));
                }
            }
            let (team, court) = REGION_REWARDS[region];
            notes.push(Notice::Court(court));
            if team != 0 && !self.unlocked_teams.contains(&team) {
                self.unlocked_teams.push(team);
                self.unlocked_teams.sort_unstable();
                notes.push(Notice::Team(team));
            }
        }
        if let Some(flags) = self.league_native_flags.as_mut() {
            *flags |= 0x80;
            if !out.won {
                *flags &= !0x40;
            } else if boss {
                *flags = (*flags & !0x80) | 0x3f;
            }
        }
        if !was_complete && self.complete_league() {
            self.league_rewarded = true;
            let rp = LEAGUE_DONE_RP[difficulty];
            self.reward_points += rp;
            notes.push(Notice::RewardPoints(rp));
        }
        self.sync_league_user();
        notes
    }

    /// The four trophies (`poster.txt` 33..36), in the Rewards page's
    /// order: Street School (every lesson passed), League Challenge (all five
    /// regional legends beaten), Be A Legend (every event cleared), and the original game
    /// Vol. 2 for all three.
    pub fn trophies(&self) -> [bool; TROPHIES] {
        let all = (1u32 << SCHOOL_LESSONS) - 1;
        let school =
            self.school_trophy || (!self.school_profile_selected && self.school & all == all);
        let league = self.league_rewarded;
        let legend = self.legend_complete();
        let legendary = self.reputation_level() == REP_MAX_LEVEL
            || self
                .careers
                .iter()
                .flatten()
                .any(|c| c.reputation / REP_LEVEL_SIZE >= i16::from(REP_MAX_LEVEL));
        [
            school,
            league,
            legend,
            school && league && legend && legendary,
        ]
    }

    /// The fourth trophy (`FUN_001ded40`, 0x1dfd88..0x1dff30, with the
    /// other three held): its bit, then card 34 into the first free jersey
    /// slot (shown only), move 44 into the first free boss-move slot (its
    /// card is 34) and card 34 as the completion's (kind 10, granted:
    /// `FUN_001e0420`), whose player is record 167. The notices, in that
    /// order; none when the card is already the player's. (Team 34, the
    /// card's legends' team, is opened with it; it is in no Choose Team
    /// grid.)
    pub fn trophy_rewards(&mut self) -> Vec<Notice> {
        if !self.trophies()[3] || self.owned.contains(&34) {
            return Vec::new();
        }
        self.owned.push(34);
        self.unlock_player(167);
        if !self.unlocked_teams.contains(&34) {
            self.unlocked_teams.push(34);
        }
        vec![Notice::Jersey(34), Notice::BossMove(44), Notice::Card(34)]
    }

    /// Marks milestone `n` reached; whether it is new.
    pub fn reach_milestone(&mut self, n: u8) -> bool {
        if usize::from(n) >= MILESTONES || self.milestones & (1 << n) != 0 {
            return false;
        }
        self.milestones |= 1 << n;
        true
    }

    /// The milestones the save's own state completes: Street School (11,
    /// 12 without a failed lesson), as the game checks them after a game.
    pub fn school_milestones(&mut self) -> Vec<Notice> {
        let all = (1u32 << SCHOOL_LESSONS) - 1;
        let mut out = Vec::new();
        if self.school & all == all {
            if self.reach_milestone(10) {
                out.push(Notice::Milestone(10));
            }
            if !self.school_failed && self.reach_milestone(11) {
                out.push(Notice::Milestone(11));
            }
        }
        out
    }

    /// A Pick Up game won by a person: its Reward Points.
    pub fn win_pickup(&mut self, difficulty: u8) -> u32 {
        let rp = PICKUP_RP[usize::from(difficulty.min(2))];
        self.reward_points += rp;
        self.pickup_wins = self.pickup_wins.saturating_add(1);
        rp
    }

    /// Native FUN_001ee630: all ordinary teams in this region are beaten.
    pub fn league_boss_open(&self, region: usize) -> bool {
        REGION_MATCHES
            .get(region)
            .is_some_and(|teams| (0..teams.len()).all(|index| self.league_done(region, index)))
    }

    /// Native FUN_001ee710: all five ordinary lists and boss bits are done.
    pub fn complete_league(&self) -> bool {
        self.region_cleared & 0b1_1111 == 0b1_1111
            && (0..REGION_MATCHES.len()).all(|region| self.league_boss_open(region))
    }

    /// Continue after completion resets the challenge (FUN_001eeb18).
    /// Lifetime trophies, unlocked rewards and balances are retained.
    pub fn restart_league(&mut self) {
        if self.complete_league() {
            self.league_played = 0;
            self.region_cleared = 0;
            self.league_lost = 0;
            if let Some(flags) = self.league_native_flags.as_mut() {
                *flags = (*flags & !0x80) | 0x3f;
            }
            self.sync_league_user();
        }
    }

    /// The extra opponent is the region's legends team, not a sixth region.
    pub fn league_opponent(region: usize, index: usize) -> Option<u16> {
        let teams = REGION_MATCHES.get(region)?;
        teams
            .get(index)
            .copied()
            .or_else(|| (index == teams.len()).then_some(REGION_REWARDS[region].0))
    }

    /// Ordinary match wins and the independent regional boss result.
    pub fn league_done(&self, region: usize, index: usize) -> bool {
        let Some(teams) = REGION_MATCHES.get(region) else {
            return false;
        };
        if index == teams.len() {
            self.region_cleared & (1 << region) != 0
        } else {
            index < teams.len()
                && (self.league_team == Some(teams[index])
                    || self.league_played & league_bit(region, index) != 0)
        }
    }

    /// A reward from the cards' development-point category (`XDBCARD` rows:
    /// 100 points for 200 Reward Points, 200 for 350, 300 for 500, 500 for
    /// 750, 1000 for 1200).
    pub fn spend_reward_points(&mut self, rp: u32, dp: u32) -> bool {
        if self.reward_points < rp {
            return false;
        }
        self.reward_points -= rp;
        self.credit_star(dp);
        true
    }

    /// Buy a card by id when the points cover it: takes the price, marks it
    /// owned, and grants what the card grants (`likely` for the un-traced
    /// grants): legends join the recruit pool and their team becomes
    /// playable.
    pub fn buy(&mut self, id: u16, cost: u32, grant: CardGrant) -> bool {
        if self.owned.contains(&id) || self.reward_points < cost {
            return false;
        }
        self.reward_points -= cost;
        self.owned.push(id);
        match grant {
            CardGrant::Player(record) => self.unlock_player(record),
            CardGrant::Court(court) => self.court_granted(court),
            CardGrant::Thing => {}
        }
        true
    }

    /// The created baller who joined league team `team` as its sixth man.
    pub fn sixth_man(&self, team: u16) -> Option<u16> {
        self.sixth_men
            .iter()
            .find(|&&(t, _)| t == team)
            .map(|&(_, r)| r)
    }

    /// `record` joins league team `team` as its sixth man (`FUN_00271cb8`):
    /// it leaves the team it joined before (`FUN_001d84d0`, then
    /// `FUN_001e0030(.., -1)`), and takes the place of the team's own.
    pub fn join_team(&mut self, team: u16, record: u16) {
        self.sixth_men.retain(|&(t, r)| t != team && r != record);
        self.sixth_men.push((team, record));
        self.sixth_men.sort_unstable();
    }

    /// A created baller's place on a league team goes with it.
    pub fn leave_team(&mut self, record: u16) {
        self.sixth_men.retain(|&(_, r)| r != record);
    }

    /// Drops whatever in a loaded save points past the disc's tables
    /// (`players` records, `teams` database teams, `moves` move rows), so a
    /// damaged or hand-edited save can't index out of them. Returns how
    /// many things went.
    pub fn sanitize(&mut self, players: usize, teams: usize, moves: usize) -> usize {
        let before = self.crew.len()
            + self.unlocked_players.len()
            + self.unlocked_teams.len()
            + self.ballers.len()
            + self.teams.len();
        let mut slots = 0u32;
        self.ballers.retain(|b| {
            let fresh = usize::from(b.slot) < STAR_SLOTS && slots & (1 << b.slot) == 0;
            if fresh {
                slots |= 1 << b.slot;
            }
            fresh
        });
        // Career stars past the disc's records exist once their baller does.
        let stars: Vec<u16> = self.ballers.iter().map(|b| b.record() as u16).collect();
        self.sixth_men
            .retain(|&(t, r)| usize::from(t) < LEAGUE_TEAMS && stars.contains(&r));
        let player = |r: &u16| usize::from(*r) < players || stars.contains(r);
        for c in self.careers.iter_mut().flatten() {
            c.crew.retain(player);
            c.crew.truncate(TEAM_SIZE);
        }
        self.crew.retain(player);
        let mut seen = Vec::new();
        self.crew.retain(|r| {
            let fresh = !seen.contains(r);
            seen.push(*r);
            fresh
        });
        self.crew.truncate(TEAM_SIZE);
        self.unlocked_players.retain(player);
        if self.league_roster.len() != 3 || self.league_roster.iter().any(|r| !player(r)) {
            self.league_roster.clear();
        }
        for user in &mut self.records.users {
            if user.league.roster.len() != 3
                || user.league.roster.iter().any(|r| !player(r))
                || user.league.roster[0] == user.league.roster[1]
                || user.league.roster[0] == user.league.roster[2]
                || user.league.roster[1] == user.league.roster[2]
            {
                user.league.roster.clear();
            }
        }
        self.baller_points.retain(|(r, _)| player(r));
        self.unlocked_teams.retain(|&t| usize::from(t) < teams);
        let mut dropped_moves = 0;
        for b in &mut self.ballers {
            b.unlocked.retain(|&m| usize::from(m) < moves);
            for (kind, list) in b.moves.iter_mut().enumerate() {
                let n = list.len();
                for row in list.iter_mut() {
                    if *row != EMPTY_MOVE_SLOT && usize::from(*row) >= moves {
                        *row = EMPTY_MOVE_SLOT;
                        dropped_moves += 1;
                    }
                }
                list.truncate(MOVE_SLOTS[kind]);
                dropped_moves += n - list.len();
            }
        }
        self.teams.retain(|t| t.roster.iter().all(player));
        self.teams.truncate(TEAM_SLOTS);
        let after = self.crew.len()
            + self.unlocked_players.len()
            + self.unlocked_teams.len()
            + self.ballers.len()
            + self.teams.len();
        before - after + dropped_moves
    }

    /// A player joins the recruit pool, and the street legends' team (29),
    /// which holds the bought crew members, becomes playable.
    pub fn unlock_player(&mut self, record: u16) {
        if !self.unlocked_players.contains(&record) {
            self.unlocked_players.push(record);
            self.unlocked_players.sort_unstable();
        }
        if !self.unlocked_teams.contains(&29) {
            self.unlocked_teams.push(29);
        }
    }
}

/// A save string: a length byte, then up to 255 bytes of UTF-8 (cut at a
/// character boundary).
fn put_name(out: &mut Vec<u8>, name: &str) {
    let mut end = name.len().min(255);
    while !name.is_char_boundary(end) {
        end -= 1;
    }
    out.push(end as u8);
    out.extend_from_slice(&name.as_bytes()[..end]);
}

/// What buying a card grants, as far as the shop can tell.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardGrant {
    /// A player record joins the pool.
    Player(u16),
    /// A court's card: the court opens (by its id).
    Court(u16),
    /// A jersey or an un-traced row: owned, nothing else.
    Thing,
}

impl Story {
    /// What owning court `court`'s card does beyond the card
    /// (`FUN_001e0420`, 0x1e0558..0x1e05b0): the Rucker Classic's (court
    /// 10) switches its song on in the options (`FUN_001d7398(options,
    /// track 20, 1)`, then the live block copied from them).
    pub fn court_granted(&mut self, court: u16) {
        if court == 10 {
            let mut s = self.settings.unwrap_or_default();
            let bit = 1 << crate::records::RUCKER_CLASSIC_SONG_BIT;
            s.songs_off &= !bit;
            self.settings = Some(s);
            self.songs_switched_on |= bit;
        }
    }

    // -- persistence -------------------------------------------------------

    /// The disk format: `SVSt`, u32 LE version 1, then the fields in order
    /// below, little-endian. Lists are u32 length then items.
    pub fn write(&self) -> Vec<u8> {
        let league = if self.league_profile_selected {
            self.league_unassigned.clone().unwrap_or_default()
        } else {
            self.league_progress()
        };
        let school = if self.school_profile_selected {
            self.school_unassigned.unwrap_or_default()
        } else {
            (self.school, self.school_failed)
        };
        let mut records = self.records.clone();
        if let Some(index) = self
            .league_user
            .as_deref()
            .and_then(|name| records.user(name))
        {
            records.users[index].league = self.league_progress();
        }
        if let Some(index) = self
            .school_user
            .as_deref()
            .and_then(|name| records.user(name))
        {
            records.users[index].school = self.school_progress();
        }
        let mut out = Vec::with_capacity(256);
        let put32 = |out: &mut Vec<u8>, n: u32| out.extend_from_slice(&n.to_le_bytes());
        out.extend_from_slice(b"SVST");
        put32(&mut out, 1);
        put32(&mut out, self.games);
        put32(&mut out, self.wins);
        put32(&mut out, self.losses);
        put32(&mut out, self.development_points);
        put32(&mut out, self.reward_points);
        out.extend_from_slice(&self.progress);
        out.extend_from_slice(&self.reputation.to_le_bytes());
        put32(&mut out, self.crew.len() as u32);
        for &member in &self.crew {
            out.extend_from_slice(&member.to_le_bytes());
        }
        for &u in &self.upgrades {
            out.extend_from_slice(&u.to_le_bytes());
        }
        out.extend_from_slice(&self.street_cleared.to_le_bytes());
        out.extend_from_slice(&self.tournament_cleared.to_le_bytes());
        out.extend_from_slice(&league.played.to_le_bytes());
        out.push(league.cleared);
        out.push(u8::from(self.nickname));
        out.push(u8::from(self.league_rewarded));
        out.push(0);
        put32(&mut out, self.unlocked_teams.len() as u32);
        for &t in &self.unlocked_teams {
            out.extend_from_slice(&t.to_le_bytes());
        }
        put32(&mut out, self.unlocked_players.len() as u32);
        for &p in &self.unlocked_players {
            out.extend_from_slice(&p.to_le_bytes());
        }
        put32(&mut out, self.owned.len() as u32);
        for &c in &self.owned {
            out.extend_from_slice(&c.to_le_bytes());
        }
        out.extend_from_slice(&school.0.to_le_bytes());
        // Optional sections follow, each a tag and its data; older saves
        // simply end before them.
        if !self.ballers.is_empty() {
            out.extend_from_slice(b"BALR");
            put32(&mut out, self.ballers.len() as u32);
            for b in &self.ballers {
                out.extend_from_slice(&[b.slot, u8::from(b.female), b.build, b.height_in]);
                out.extend_from_slice(&b.weight_lb.to_le_bytes());
                out.extend_from_slice(&[
                    b.look,
                    b.hair_style as u8,
                    b.hair_color,
                    b.facial_hair_style as u8,
                    b.facial_hair_color,
                    b.clothing,
                    b.clothing_variant,
                    b.shoes,
                ]);
                out.extend_from_slice(&b.skills);
                for name in [&b.first_name, &b.last_name] {
                    put_name(&mut out, name);
                }
            }
        }
        if !self.teams.is_empty() {
            out.extend_from_slice(b"TEAM");
            put32(&mut out, self.teams.len() as u32);
            for t in &self.teams {
                put_name(&mut out, &t.city);
                put_name(&mut out, &t.name);
                out.push(t.logo);
                for r in t.roster {
                    out.extend_from_slice(&r.to_le_bytes());
                }
            }
        }
        // Records and options: a tag, a length, the section's own format.
        let section = |out: &mut Vec<u8>, tag: &[u8; 4], body: Vec<u8>| {
            out.extend_from_slice(tag);
            put32(out, body.len() as u32);
            out.extend_from_slice(&body);
        };
        if records != crate::records::Records::default() {
            section(&mut out, b"RECS", records.encode());
        }
        if let Some(settings) = self.settings {
            section(&mut out, b"OPTS", settings.encode());
        }
        if !self.brackets.is_empty() {
            let mut body = vec![self.brackets.len() as u8];
            for (ev, b) in &self.brackets {
                body.push(*ev);
                b.encode(&mut body);
            }
            section(&mut out, b"BRKT", body);
        }
        // The careers (the one being played as it stands).
        if self.careers.iter().any(Option::is_some) || self.active.is_some() {
            let mut body = vec![self.active.unwrap_or(0xff)];
            for slot in 0..CAREER_SLOTS {
                match self.career_at(slot) {
                    Some(c) => {
                        body.push(1);
                        c.encode(&mut body);
                    }
                    None => body.push(0),
                }
            }
            section(&mut out, b"CARS", body);
        }
        if self.pickup_wins != 0 {
            section(&mut out, b"PKUP", self.pickup_wins.to_le_bytes().to_vec());
        }
        // The milestones and what they track.
        if self.milestones != 0 || league.lost != 0 || school.1 || self.modes_played != 0 {
            let mut body = self.milestones.to_le_bytes().to_vec();
            body.extend([league.lost, u8::from(school.1), self.modes_played]);
            section(&mut out, b"MILE", body);
        }
        // The moves each baller has unlocked.
        let unlocked: Vec<&CreatedBaller> = self
            .ballers
            .iter()
            .filter(|b| !b.unlocked.is_empty())
            .collect();
        if !unlocked.is_empty() {
            let mut body = vec![unlocked.len() as u8];
            for b in unlocked {
                let n = b.unlocked.len().min(255);
                body.push(b.slot);
                body.push(n as u8);
                body.extend_from_slice(&b.unlocked[..n]);
            }
            section(&mut out, b"UNLK", body);
        }
        // The ballers' moves, after the sections older versions know.
        let with_moves: Vec<&CreatedBaller> = self
            .ballers
            .iter()
            .filter(|b| b.moves.iter().any(|m| !m.is_empty()))
            .collect();
        if !with_moves.is_empty() {
            out.extend_from_slice(b"MOVE");
            put32(&mut out, with_moves.len() as u32);
            for b in with_moves {
                out.push(b.slot);
                for (kind, list) in b.moves.iter().enumerate() {
                    let n = list.len().min(MOVE_SLOTS[kind]);
                    out.push(n as u8);
                    out.extend_from_slice(&list[..n]);
                }
            }
        }
        if !self.baller_points.is_empty() {
            let mut body = Vec::new();
            put32(&mut body, self.baller_points.len() as u32);
            for &(record, points) in &self.baller_points {
                body.extend_from_slice(&record.to_le_bytes());
                put32(&mut body, points);
            }
            section(&mut out, b"DPTS", body);
        }
        if !league.roster.is_empty() {
            let body = league.roster.iter().flat_map(|r| r.to_le_bytes()).collect();
            section(&mut out, &LEAGUE_ROSTER, body);
        }
        // Earlier Rust saves cleared a region after its last ordinary team
        // and invented a sixth match. This marks actual regional boss bits.
        section(&mut out, &LEAGUE_BOSSES, vec![1]);
        section(&mut out, b"SCTR", vec![1, u8::from(self.school_trophy)]);
        // Explicit row-index semantics, including the active nickname in
        // saves without a CARS section. Older Rust saves stored field-0 ids.
        let mut nickname_row = vec![1];
        nickname_row.extend_from_slice(&self.nickname_id.to_le_bytes());
        section(&mut out, b"NKRW", nickname_row);
        let pending_mask = (0..CAREER_SLOTS).fold(0u16, |mask, slot| {
            let pending = if self.active == Some(slot as u8) {
                self.nickname_pending
            } else {
                self.careers[slot]
                    .as_ref()
                    .is_some_and(|c| c.nickname_pending)
            };
            mask | (u16::from(pending) << slot)
        });
        if self.nickname_pending || pending_mask != 0 {
            let mut body = vec![1, u8::from(self.nickname_pending)];
            body.extend_from_slice(&pending_mask.to_le_bytes());
            section(&mut out, b"NKPN", body);
        }
        // What each baller has bought to wear (`cab::Wardrobe`); written
        // last, so a build that does not know it keeps everything before.
        if !self.ballers.is_empty() {
            let mut body = vec![self.ballers.len() as u8];
            for b in &self.ballers {
                body.push(b.slot);
                let w = &b.wardrobe;
                for word in [w.outfits, w.colours, w.shoes] {
                    body.extend_from_slice(&word.to_le_bytes());
                }
            }
            section(&mut out, b"WARD", body);
        }
        // The careers' teams (`CrewTeam`), past every older section.
        let teams: Vec<(usize, CrewTeam)> = (0..CAREER_SLOTS)
            .filter_map(|slot| Some((slot, self.career_at(slot)?.team)))
            .filter(|(_, t)| *t != CrewTeam::default())
            .collect();
        if !teams.is_empty() {
            let mut body = vec![teams.len() as u8];
            for (slot, t) in &teams {
                body.push(*slot as u8);
                put_name(&mut body, &t.city);
                put_name(&mut body, &t.name);
                body.push(t.logo);
            }
            section(&mut out, b"CTEM", body);
        }
        // The ballers' icons, past the careers' teams.
        if self.ballers.iter().any(|b| b.icon != 0) {
            let mut body = vec![self.ballers.len() as u8];
            for b in &self.ballers {
                body.extend_from_slice(&[b.slot, b.icon]);
            }
            section(&mut out, b"BICN", body);
        }
        // The league teams' sixth men, past the icons.
        if !self.sixth_men.is_empty() {
            let mut body = vec![self.sixth_men.len() as u8];
            for &(team, record) in &self.sixth_men {
                body.push(team as u8);
                body.extend_from_slice(&record.to_le_bytes());
            }
            section(&mut out, b"SIXM", body);
        }
        out
    }

    /// Reads [`Self::write`]'s format; a wrong magic or a bitten-off short
    /// buffer is a `None` (the caller starts fresh).
    pub fn read(data: &[u8]) -> Option<Self> {
        let take = |at: &mut usize, n: usize| -> Option<&[u8]> {
            let out = data.get(*at..*at + n)?;
            *at += n;
            Some(out)
        };
        let get32 = |at: &mut usize| -> Option<u32> {
            take(at, 4).map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        };
        let mut s = Story::default();
        let mut at = 0usize;
        if take(&mut at, 4)? != b"SVST" || get32(&mut at)? != 1 {
            return None;
        }
        s.games = get32(&mut at)?;
        s.wins = get32(&mut at)?;
        s.losses = get32(&mut at)?;
        s.development_points = get32(&mut at)?;
        s.reward_points = get32(&mut at)?;
        s.progress.take_from(take(&mut at, EVENTS.len())?)?;
        s.reputation = i16::from_le_bytes(take(&mut at, 2)?.try_into().unwrap());
        let crew = get32(&mut at)? as usize;
        if crew > 4096 {
            return None;
        }
        s.crew = (0..crew)
            .map(|_| {
                let b: [u8; 2] = take(&mut at, 2)?.try_into().unwrap();
                Some(u16::from_le_bytes(b))
            })
            .collect::<Option<Vec<_>>>()?;
        s.upgrades.take_from(take(&mut at, SKILLS.len() * 2)?)?;
        s.street_cleared = get32(&mut at)?;
        s.tournament_cleared = get32(&mut at)?;
        s.league_played = u64::from(get32(&mut at)?) | u64::from(get32(&mut at)?) << 32;
        s.region_cleared = take(&mut at, 1)?[0];
        s.nickname = take(&mut at, 1)?[0] != 0;
        s.league_rewarded = take(&mut at, 1)?[0] != 0;
        at += 1;
        let teams = get32(&mut at)? as usize;
        if teams > 1024 {
            return None;
        }
        s.unlocked_teams = (0..teams)
            .map(|_| {
                let b: [u8; 2] = take(&mut at, 2)?.try_into().unwrap();
                Some(u16::from_le_bytes(b))
            })
            .collect::<Option<Vec<_>>>()?;
        let players = get32(&mut at)? as usize;
        if players > 4096 {
            return None;
        }
        s.unlocked_players = (0..players)
            .map(|_| {
                let b: [u8; 2] = take(&mut at, 2)?.try_into().unwrap();
                Some(u16::from_le_bytes(b))
            })
            .collect::<Option<Vec<_>>>()?;
        let owned = get32(&mut at)? as usize;
        if owned > 4096 {
            return None;
        }
        s.owned = (0..owned)
            .map(|_| {
                let b: [u8; 2] = take(&mut at, 2)?.try_into().unwrap();
                Some(u16::from_le_bytes(b))
            })
            .collect::<Option<Vec<_>>>()?;
        s.school = u32::from_le_bytes(take(&mut at, 4)?.try_into().unwrap());
        let mut pending_unlocked: Vec<(u8, Vec<u8>)> = Vec::new();
        let mut wardrobes: Option<Vec<(u8, crate::cab::Wardrobe)>> = None;
        let mut had_careers = false;
        let mut nickname_rows = false;
        let mut nickname_pending = None;
        let mut regional_bosses = false;
        let mut school_trophy = false;
        let mut crew_teams: Vec<(usize, CrewTeam)> = Vec::new();
        let mut icons: Vec<(u8, u8)> = Vec::new();
        while let Some(tag) = take(&mut at, 4) {
            match tag {
                b"SCTR" => {
                    if get32(&mut at)? != 2 {
                        return None;
                    }
                    let body = take(&mut at, 2)?;
                    if body[0] != 1 || body[1] > 1 || school_trophy {
                        return None;
                    }
                    s.school_trophy = body[1] != 0;
                    school_trophy = true;
                }
                t if *t == LEAGUE_BOSSES || *t == LEGACY_LEAGUE_BOSSES => {
                    if get32(&mut at)? != 1 || take(&mut at, 1)?[0] != 1 {
                        return None;
                    }
                    regional_bosses = true;
                }
                b"NKPN" => {
                    if get32(&mut at)? != 4 {
                        return None;
                    }
                    let body = take(&mut at, 4)?;
                    let mask = u16::from_le_bytes(body[2..4].try_into().unwrap());
                    if body[0] != 1 || body[1] > 1 || mask >> CAREER_SLOTS != 0 {
                        return None;
                    }
                    nickname_pending = Some((body[1] != 0, mask));
                }
                b"NKRW" => {
                    if get32(&mut at)? != 3 {
                        return None;
                    }
                    let body = take(&mut at, 3)?;
                    if body[0] != 1 {
                        return None;
                    }
                    s.nickname_id = i16::from_le_bytes(body[1..3].try_into().unwrap());
                    nickname_rows = true;
                }
                t if *t == LEAGUE_ROSTER || *t == LEGACY_LEAGUE_ROSTER => {
                    let length = get32(&mut at)? as usize;
                    if length != 6 {
                        return None;
                    }
                    let body = take(&mut at, length)?;
                    let roster: Vec<u16> = body
                        .as_chunks::<2>()
                        .0
                        .iter()
                        .map(|b| u16::from_le_bytes(*b))
                        .collect();
                    if !s.select_league_roster(&roster) {
                        return None;
                    }
                }
                b"DPTS" => {
                    let length = get32(&mut at)? as usize;
                    let body = take(&mut at, length)?;
                    let n = u32::from_le_bytes(body.get(..4)?.try_into().ok()?) as usize;
                    if n > 4096 || length != 4 + n * 6 {
                        return None;
                    }
                    for entry in body[4..].as_chunks::<6>().0 {
                        let record = u16::from_le_bytes(entry[..2].try_into().ok()?);
                        if s.baller_points.iter().any(|(r, _)| *r == record) {
                            return None;
                        }
                        let points = u32::from_le_bytes(entry[2..].try_into().ok()?);
                        s.baller_points.push((record, points));
                    }
                }
                b"BALR" => {
                    let n = get32(&mut at)? as usize;
                    if n > STAR_SLOTS {
                        return None;
                    }
                    for _ in 0..n {
                        let head = take(&mut at, 4)?;
                        let weight = u16::from_le_bytes(take(&mut at, 2)?.try_into().unwrap());
                        let look = take(&mut at, 8)?;
                        let skills: [u8; 7] = take(&mut at, 7)?.try_into().unwrap();
                        let mut name = || -> Option<String> {
                            let len = usize::from(take(&mut at, 1)?[0]);
                            Some(String::from_utf8_lossy(take(&mut at, len)?).into_owned())
                        };
                        let (first_name, last_name) = (name()?, name()?);
                        s.ballers.push(CreatedBaller {
                            slot: head[0],
                            female: head[1] != 0,
                            build: head[2],
                            height_in: head[3],
                            weight_lb: weight,
                            look: look[0],
                            hair_style: look[1] as i8,
                            hair_color: look[2],
                            facial_hair_style: look[3] as i8,
                            facial_hair_color: look[4],
                            clothing: look[5],
                            clothing_variant: look[6],
                            shoes: look[7],
                            skills,
                            first_name,
                            last_name,
                            moves: Default::default(),
                            unlocked: Vec::new(),
                            wardrobe: Default::default(),
                            icon: 0,
                        });
                    }
                }
                b"TEAM" => {
                    let n = get32(&mut at)? as usize;
                    if n > TEAM_SLOTS {
                        return None;
                    }
                    for _ in 0..n {
                        let mut name = || -> Option<String> {
                            let len = usize::from(take(&mut at, 1)?[0]);
                            Some(String::from_utf8_lossy(take(&mut at, len)?).into_owned())
                        };
                        let (city, name) = (name()?, name()?);
                        let logo = take(&mut at, 1)?[0];
                        let mut roster = [0u16; TEAM_PLAYERS];
                        for r in &mut roster {
                            *r = u16::from_le_bytes(take(&mut at, 2)?.try_into().unwrap());
                        }
                        s.teams.push(CreatedTeam {
                            city,
                            name,
                            logo,
                            roster,
                        });
                    }
                }
                b"RECS" | b"OPTS" | b"BRKT" | b"UNLK" | b"CARS" | b"MILE" | b"PKUP" => {
                    let n = get32(&mut at)? as usize;
                    let body = take(&mut at, n)?;
                    if tag == b"PKUP" {
                        s.pickup_wins = u32::from_le_bytes(body.try_into().ok()?);
                    } else if tag == b"MILE" {
                        let b = body.get(..7)?;
                        s.milestones = u32::from_le_bytes(b[..4].try_into().unwrap());
                        s.league_lost = b[4];
                        s.school_failed = b[5] != 0;
                        s.modes_played = b[6];
                    } else if tag == b"CARS" {
                        let active = *body.first()?;
                        let mut rest = &body[1..];
                        for slot in 0..CAREER_SLOTS {
                            let present = *rest.first()?;
                            rest = &rest[1..];
                            if present != 0 {
                                let (c, used) = Career::decode(rest)?;
                                rest = &rest[used..];
                                s.careers[slot] = Some(c);
                            }
                        }
                        if usize::from(active) < CAREER_SLOTS
                            && s.careers[usize::from(active)].is_some()
                        {
                            s.active = Some(active);
                        }
                        had_careers = true;
                    } else if tag == b"UNLK" {
                        let count = usize::from(*body.first()?);
                        let mut rest = &body[1..];
                        for _ in 0..count {
                            let (slot, n) = (*rest.first()?, usize::from(*rest.get(1)?));
                            let rows = rest.get(2..2 + n)?.to_vec();
                            rest = &rest[2 + n..];
                            pending_unlocked.push((slot, rows));
                        }
                    } else if tag == b"RECS" {
                        s.records = crate::records::Records::decode(body)?;
                    } else if tag == b"OPTS" {
                        s.settings = Some(crate::records::Settings::decode(body)?);
                    } else {
                        let count = usize::from(*body.first()?);
                        let mut rest = &body[1..];
                        for _ in 0..count {
                            let ev = *rest.first()?;
                            if usize::from(ev) >= EVENTS.len() {
                                return None;
                            }
                            let (b, used) = crate::bracket::Bracket::decode(&rest[1..])?;
                            s.brackets.push((ev, b));
                            rest = &rest[1 + used..];
                        }
                    }
                }
                b"MOVE" => {
                    let n = get32(&mut at)? as usize;
                    if n > STAR_SLOTS {
                        return None;
                    }
                    for _ in 0..n {
                        let slot = take(&mut at, 1)?[0];
                        let mut moves: [Vec<u8>; 3] = Default::default();
                        for (kind, list) in moves.iter_mut().enumerate() {
                            let count = usize::from(take(&mut at, 1)?[0]);
                            if count > MOVE_SLOTS[kind] {
                                return None;
                            }
                            *list = take(&mut at, count)?.to_vec();
                        }
                        if let Some(b) = s.ballers.iter_mut().find(|b| b.slot == slot) {
                            b.moves = moves;
                        }
                    }
                }
                b"WARD" => {
                    let n = get32(&mut at)? as usize;
                    let body = take(&mut at, n)?;
                    let count = usize::from(*body.first()?);
                    if count > STAR_SLOTS || n != 1 + count * 13 {
                        return None;
                    }
                    let word = |e: &[u8], k: usize| {
                        u32::from_le_bytes(e[1 + 4 * k..5 + 4 * k].try_into().unwrap())
                    };
                    wardrobes = Some(
                        body[1..]
                            .chunks(13)
                            .map(|e| {
                                let wardrobe = crate::cab::Wardrobe {
                                    outfits: word(e, 0),
                                    colours: word(e, 1),
                                    shoes: word(e, 2),
                                };
                                (e[0], wardrobe)
                            })
                            .collect(),
                    );
                }
                b"CTEM" => {
                    let n = get32(&mut at)? as usize;
                    let body = take(&mut at, n)?;
                    let count = usize::from(*body.first()?);
                    let mut rest = &body[1..];
                    let name = |rest: &mut &[u8]| -> Option<String> {
                        let len = usize::from(*rest.first()?);
                        let s = String::from_utf8_lossy(rest.get(1..1 + len)?).into_owned();
                        *rest = &rest[1 + len..];
                        Some(s)
                    };
                    for _ in 0..count {
                        let slot = usize::from(*rest.first()?);
                        rest = &rest[1..];
                        let city = name(&mut rest)?;
                        let team = name(&mut rest)?;
                        let logo = *rest.first()?;
                        rest = &rest[1..];
                        crew_teams.push((
                            slot,
                            CrewTeam {
                                city,
                                name: team,
                                logo,
                            },
                        ));
                    }
                }
                b"BICN" => {
                    let n = get32(&mut at)? as usize;
                    let body = take(&mut at, n)?;
                    let count = usize::from(*body.first()?);
                    if count > STAR_SLOTS || n != 1 + count * 2 {
                        return None;
                    }
                    icons = body[1..].chunks(2).map(|e| (e[0], e[1].min(6))).collect();
                }
                b"SIXM" => {
                    let n = get32(&mut at)? as usize;
                    let body = take(&mut at, n)?;
                    let count = usize::from(*body.first()?);
                    if count > LEAGUE_TEAMS || n != 1 + count * 3 {
                        return None;
                    }
                    s.sixth_men = body[1..]
                        .chunks(3)
                        .map(|e| (u16::from(e[0]), u16::from_le_bytes([e[1], e[2]])))
                        .collect();
                }
                // A section from a newer version: nothing after it is read.
                _ => break,
            }
        }
        match wardrobes {
            Some(list) => {
                for (slot, wardrobe) in list {
                    if let Some(b) = s.ballers.iter_mut().find(|b| b.slot == slot) {
                        b.wardrobe = wardrobe;
                    }
                }
            }
            // A save from before the wardrobe paid for what each baller
            // wears (it charged the worn outfit and shoes): they are owned.
            None => {
                for b in &mut s.ballers {
                    b.wardrobe = b.worn_wardrobe();
                }
            }
        }
        for (slot, rows) in pending_unlocked {
            if let Some(b) = s.ballers.iter_mut().find(|b| b.slot == slot) {
                b.unlocked = rows;
            }
        }
        for (slot, icon) in icons {
            if let Some(b) = s.ballers.iter_mut().find(|b| b.slot == slot) {
                b.icon = icon;
            }
        }
        for (slot, team) in crew_teams {
            if s.active == Some(slot as u8) {
                s.crew_team = team.clone();
            }
            if let Some(Some(c)) = s.careers.get_mut(slot) {
                c.team = team;
            }
        }
        if !school_trophy {
            s.school_trophy = s.school & ((1 << SCHOOL_LESSONS) - 1) == (1 << SCHOOL_LESSONS) - 1;
        }
        if !regional_bosses {
            // Preserve everything already earned. Incomplete old runs still
            // need their bosses; a completed old challenge keeps completion.
            s.region_cleared = if s.league_rewarded { 0b1_1111 } else { 0 };
            if s.league_rewarded {
                for (region, teams) in REGION_MATCHES.iter().enumerate() {
                    for index in 0..teams.len() {
                        s.league_played |= league_bit(region, index);
                    }
                }
            }
            s.league_played &= (1u64 << 40) - 1;
        }
        if !nickname_rows {
            // Map the old Rust save's database ids to the actual alias row
            // numbers. Original console imports already carry row numbers
            // and get the marker on their first Rust-format write.
            let migrate = |earned: &mut bool, id: &mut i16| {
                if *earned {
                    if let Some(row) = legacy_nickname_row(*id) {
                        *id = row;
                    } else {
                        *earned = false;
                        *id = -1;
                    }
                }
            };
            migrate(&mut s.nickname, &mut s.nickname_id);
            for career in s.careers.iter_mut().flatten() {
                migrate(&mut career.nickname, &mut career.nickname_id);
            }
        }
        if let Some((pending, mask)) = nickname_pending {
            s.nickname_pending = pending;
            for slot in 0..CAREER_SLOTS {
                if let Some(career) = &mut s.careers[slot] {
                    career.nickname_pending = mask & (1 << slot) != 0;
                } else if mask & (1 << slot) != 0 {
                    return None;
                }
            }
            if had_careers && s.active.is_some_and(|a| pending != (mask & (1 << a) != 0)) {
                return None;
            }
        }
        if had_careers {
            // The career being played comes out of its slot.
            if let Some(a) = s.active
                && let Some(c) = s.careers[usize::from(a)].clone()
            {
                s.set_career(c);
            }
        } else if !s.crew.is_empty() {
            // A save from before careers: its progress is career 1.
            s.careers[0] = Some(s.career());
            s.active = Some(0);
        }
        // An older shared pool belonged to its current career's baller.
        // Without a career, choose the first saved baller deterministically.
        if s.baller_points.is_empty()
            && s.development_points > 0
            && let Some(record) = s
                .crew
                .first()
                .copied()
                .or_else(|| s.ballers.first().map(|b| b.record() as u16))
        {
            s.credit_baller(record, 0);
        }
        s.own_cleared_cards();
        Some(s)
    }

    /// The cards every cleared event grants (a street challenge's jersey,
    /// a tournament's boss card), for saves made before the rewrite
    /// granted them; in every career.
    fn own_cleared_cards(&mut self) {
        let clears = std::iter::once((self.tournament_cleared, self.street_cleared)).chain(
            self.careers
                .iter()
                .flatten()
                .map(|c| (c.tournament_cleared, c.street_cleared)),
        );
        let mut cards = Vec::new();
        for (tournaments, streets) in clears {
            for (ev, event) in EVENTS.iter().enumerate() {
                let card = match event.kind {
                    EventKind::Tournament if tournaments & (1 << ev) != 0 => STREET_CARD_RECORDS
                        .iter()
                        .position(|&r| r == rival_of(ev))
                        .map(|k| k as u16),
                    EventKind::StreetChallenge
                        if streets & (1 << street_ordinal(ev)) != 0
                            && (0x41..=0x46).contains(&event.boss_team) =>
                    {
                        Some(event.boss_team)
                    }
                    _ => None,
                };
                cards.extend(card);
            }
        }
        for card in cards {
            if !self.owned.contains(&card) {
                self.owned.push(card);
            }
        }
    }
}

/// Field-0 ids of the USA disc's 48 XDBALIAS rows. These values are used
/// solely to migrate old Rust saves that incorrectly stored those ids.
fn legacy_nickname_row(id: i16) -> Option<i16> {
    const IDS: [i16; 48] = [
        1, 2, 4, 5, 6, 7, 8, 9, 11, 13, 14, 15, 16, 17, 18, 51, 20, 21, 22, 24, 25, 26, 27, 28, 29,
        30, 31, 32, 34, 35, 37, 38, 39, 40, 41, 42, 43, 44, 45, 47, 49, 50, 52, 53, 54, 55, 56, 57,
    ];
    IDS.iter()
        .position(|&value| value == id)
        .map(|row| row as i16)
}

// Filling an array out of the save's bytes.
trait TakeFrom {
    fn take_from(&mut self, bytes: &[u8]) -> Option<()>;
}

impl<const N: usize> TakeFrom for [u8; N] {
    fn take_from(&mut self, b: &[u8]) -> Option<()> {
        if b.len() == N {
            self.copy_from_slice(b);
            Some(())
        } else {
            None
        }
    }
}

impl TakeFrom for [i16; 9] {
    fn take_from(&mut self, b: &[u8]) -> Option<()> {
        if b.len() != 18 {
            return None;
        }
        for (k, slot) in self.iter_mut().enumerate() {
            *slot = i16::from_le_bytes([b[k * 2], b[k * 2 + 1]]);
        }
        Some(())
    }
}

/// Every progress point an event can hold, for the menus.
pub fn event_target(ev: usize) -> i32 {
    i32::from(EVENTS[ev].target).max(0)
}

/// The tables and their consumers against the shipped program.
#[cfg(test)]
#[path = "story_parity.rs"]
mod parity;

#[cfg(test)]
mod tests {
    use super::*;

    fn outcome(won: bool, tricks: u32, l2: u8) -> MatchOutcome {
        MatchOutcome {
            won,
            trick_points: tricks,
            level2_gamebreakers: l2,
            difficulty: 1,
            ..Default::default()
        }
    }

    #[test]
    fn the_checksum_settles_identical_games() {
        use crate::{Hoop, Input, Player, Rules, World};
        // Same seed and inputs, same checksum: two freshly built worlds.
        let hoops = [
            Hoop {
                rim: [37.0, 11.75, 0.0],
                rim_radius: 0.75,
            },
            Hoop {
                rim: [-37.0, 11.75, 0.0],
                rim_radius: 0.75,
            },
        ];
        let roster = || {
            (0..6)
                .map(|k| Player::new(k % 2, [k as f32 * 3.0 - 6.0, 0.0], 0.0))
                .collect()
        };
        let a = World::new(hoops, roster(), Rules::default(), 7);
        let b = World::new(hoops, roster(), Rules::default(), 7);
        assert_eq!(a.checksum(), b.checksum());
        // And a stepped pair still agrees.
        let mut a = a;
        let mut b = b;
        let inputs = [Input::default(); 6];
        a.step(&inputs);
        b.step(&inputs);
        assert_eq!(a.checksum(), b.checksum());
        // Different placements: a different checksum.
        let shifted = (0..6)
            .map(|k| Player::new(k % 2, [k as f32 * 3.0 - 5.5, 0.0], 0.0))
            .collect();
        let c = World::new(hoops, shifted, Rules::default(), 7);
        assert_ne!(a.checksum(), c.checksum());
    }

    #[test]
    fn the_progress_shown_counts_the_reputation_levels_too() {
        // `FUN_001e74c0`: the seventeen events' targets (450) and
        // `DAT_00460a78`'s 0, 10, 10, 10, 10, 10; the original's screens
        // read "Progress: 0/500" on a new career.
        assert_eq!(overall_total(), 500);
        assert_eq!(overall_total() - OVERALL_TARGET, 50);
    }

    /// `FUN_001e91a0`: pick-ups and tournaments play the put-back Normal
    /// rules (no boss brings his own), street challenges their row of
    /// `0x460970`; the difficulty and cheats are the player's.
    #[test]
    fn an_events_rules_are_the_put_back_ones_or_its_street_row() {
        use crate::{Difficulty, Goal, Scoring};
        let mut base = crate::Rules::mode(crate::Mode::DunksOnly);
        base.difficulty = Difficulty::Easy;
        base.no_counters = true;
        for (ev, event) in EVENTS.iter().enumerate() {
            let r = event_rules(ev, &base);
            assert_eq!(r.difficulty, Difficulty::Easy);
            assert!(r.no_counters);
            if event.kind != EventKind::StreetChallenge {
                assert_eq!(r.game_type(), 0, "event {ev}");
                assert_eq!((r.goal, r.shot_clock), (Goal::Points(21), Some(24.0)));
            }
        }
        let by_court = |court: u16| {
            let ev = (0..EVENTS.len())
                .find(|&e| EVENTS[e].kind == EventKind::StreetChallenge && EVENTS[e].court == court)
                .unwrap();
            event_rules(ev, &base)
        };
        assert_eq!(by_court(1).goal, Goal::TrickPoints(250_000));
        let foster = by_court(2);
        assert!(foster.dunks_only);
        assert_eq!(
            (foster.goal, foster.shot_clock),
            (Goal::Points(12), Some(12.0))
        );
        assert_eq!(by_court(3).scoring, Scoring::OldSchool);
        assert_eq!(by_court(3).goal, Goal::Points(12));
        // Mosswood: the first Level 2 gamebreaker wins (the row's 1).
        assert_eq!(by_court(5).goal, Goal::Gamebreaker(2));
        assert_eq!(by_court(7).game_type(), 3);
        assert_eq!(
            (by_court(8).scoring, by_court(8).goal),
            (Scoring::League, Goal::Points(50))
        );
        assert_eq!(
            (by_court(10).scoring, by_court(10).goal),
            (Scoring::OldSchool, Goal::Points(21))
        );
    }

    #[test]
    fn the_78_challenge_opens_at_450() {
        let mut s = Story::default();
        // Clear the pick-ups a game at a time (their per-game value fills
        // the target), then win each tournament's whole bracket.
        for ev in [0usize, 1, 4, 6] {
            while !s.cleared(ev) {
                s.play(ev, &outcome(true, 1, 0));
            }
        }
        for ev in [2usize, 3, 5, 7, 8, 9] {
            for _ in 0..6 {
                if !s.cleared(ev) {
                    s.play(ev, &outcome(true, 5_000, 0));
                }
            }
            assert!(s.cleared(ev), "event {ev} not cleared");
        }
        // The events' 400 plus the reputation won on the way; the final
        // grind into reaching 450 can take more wins.
        for (ev, e) in EVENTS.iter().enumerate() {
            if e.target > 0 && e.kind == EventKind::Pickup && !s.cleared(ev) {
                while !s.cleared(ev) {
                    s.play(ev, &outcome(true, 1, 0));
                }
            }
        }
        while s.overall_points() < OVERALL_TARGET && s.reputation_level() < REP_MAX_LEVEL {
            s.play(9, &outcome(true, 3_000_005, 0)); // big wins grow reputation
        }
        assert!(
            s.overall_points() >= OVERALL_TARGET,
            "{} overall",
            s.overall_points()
        );
        assert!(s.open(10), "the '78 challenge opens at the target");
    }

    #[test]
    fn a_first_win_pays_the_chart_minimum() {
        let mut s = Story::default();
        let notes = s.play(0, &outcome(true, 3, 0));
        // 25 trick chart + 150 Mad Game win = 175 DP; 3 progress points.
        assert_eq!(
            notes.iter().find(|n| matches!(n, Notice::Development(_))),
            Some(&Notice::Development(175))
        );
        assert!(matches!(
            notes.iter().find(|n| matches!(n, Notice::Progress(_))),
            Some(Notice::Progress(3))
        ));
        assert_eq!(s.games, 2);
        assert_eq!(s.week(), 1);
        assert_eq!(s.wins, 1);
    }

    #[test]
    fn a_loss_pays_the_charts_but_no_win_bonus_or_progress() {
        let mut s = Story::default();
        let notes = s.play(0, &outcome(false, 8, 0));
        // The trick chart's 25 alone: no win bonus, no halving (+0xcf0 is
        // never read); a lost pick-up pays no progress.
        assert_eq!(s.development_points, 25);
        assert_eq!(s.games, 2);
        assert_eq!(s.week(), 1);
        assert_eq!(s.losses, 1);
        assert_eq!(s.progress[0], 0);
        assert!(!notes.iter().any(|n| matches!(n, Notice::Progress(_))));
    }

    #[test]
    fn story_calendar_games_and_weeks_advance_by_event_type() {
        let mut s = Story::default();
        assert_eq!((s.games, s.week()), (0, 1));

        // 1. Pickup game adds 2 calendar games.
        s.play(0, &outcome(true, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (2, 1, 0, 1));

        // Another pickup loss adds 2 calendar games.
        s.play(0, &outcome(false, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (4, 1, 1, 1));

        // A third pickup moves games to 6 (week 2).
        s.play(0, &outcome(true, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (6, 2, 1, 2));

        // 2. Street Challenge adds 2 calendar games.
        let challenge_ev = EVENTS
            .iter()
            .position(|e| e.kind == EventKind::StreetChallenge)
            .unwrap();
        s.play(challenge_ev, &outcome(true, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (8, 3, 1, 2));

        // 3. Tournament: intermediate games do not advance calendar games.
        let tourn_ev = EVENTS
            .iter()
            .position(|e| e.kind == EventKind::Tournament)
            .unwrap();
        // Round 1 win:
        let notes1 = s.play(tourn_ev, &outcome(true, 0, 0));
        assert!(notes1.iter().any(|n| matches!(n, Notice::NextRound { .. })));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (8, 4, 1, 2));

        // Round 2 win:
        let notes2 = s.play(tourn_ev, &outcome(true, 0, 0));
        assert!(notes2.iter().any(|n| matches!(n, Notice::NextRound { .. })));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (8, 5, 1, 2));

        // Round 3 win (Championship): tournament completed, adds 6 calendar games!
        let _notes3 = s.play(tourn_ev, &outcome(true, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (14, 6, 1, 3));

        // Tournament elimination: loss knocks out and adds 6 calendar games.
        s.play(tourn_ev, &outcome(false, 0, 0));
        assert_eq!((s.games, s.wins, s.losses, s.week()), (20, 6, 2, 4));
    }

    #[test]
    fn the_breakdown_lists_each_chart_by_difficulty() {
        let lines = |difficulty, l1, l2, won| {
            Story::chart_lines(&MatchOutcome {
                won,
                trick_points: 1_000_000,
                level1_gamebreakers: l1,
                level2_gamebreakers: l2,
                difficulty,
            })
        };
        let d = lines(0, 3, 1, true);
        assert_eq!(
            (d.tricks, d.level1, d.level2, d.win_loss),
            (125, 30, 25, 100)
        );
        // The charts stop at their caps: 100/250/500 and 225/450/900.
        let d = lines(2, 20, 20, true);
        assert_eq!((d.level1, d.level2, d.win_loss), (500, 900, 200));
        let d = lines(1, 2, 0, false);
        assert_eq!((d.tricks, d.level1, d.level2, d.win_loss), (125, 50, 0, 0));
        // The trick chart's first row reached wins: 1,000,001 pays 150.
        let d = Story::chart_lines(&MatchOutcome {
            trick_points: 1_000_001,
            ..Default::default()
        });
        assert_eq!(d.tricks, 150);
    }

    #[test]
    fn finishing_a_pickup_pays_the_completed_court() {
        let mut s = Story::default();
        let target = EVENTS[0].target as u32;
        let per = u32::from(EVENTS[0].points_per_game);
        let mut last = None;
        for _ in 0..target.div_ceil(per) {
            last = s
                .play(0, &outcome(true, 3, 0))
                .into_iter()
                .find_map(|n| match n {
                    Notice::DevBreakdown(d) => Some(d),
                    _ => None,
                });
        }
        assert!(s.cleared(0));
        assert_eq!(last.map(|d| d.court), Some(COURT_DP));
        // Once complete, no more court line.
        let again = s.play(0, &outcome(true, 3, 0));
        assert!(
            again
                .iter()
                .all(|n| !matches!(n, Notice::DevBreakdown(d) if d.court > 0))
        );
    }

    #[test]
    fn reputation_rises_to_the_earned_level() {
        let mut s = Story::default();
        // A level 4 event (Rucker Park tournament) at Rookie pays +5 a win.
        s.play(9, &outcome(true, 100, 0));
        assert_eq!(s.reputation, 5);
        s.play(9, &outcome(true, 100, 0));
        assert_eq!(s.reputation, 10);
        assert_eq!(s.reputation_level(), 0);
        let notes = s.play(9, &outcome(true, 100, 0));
        assert!(
            notes.contains(&Notice::Reputation(1)),
            "third win crosses 11 points: {notes:?}"
        );
        assert!(notes.contains(&Notice::Reputation(1)));
    }

    #[test]
    fn a_careers_team_name_and_logo_survive_the_save() {
        let mut s = Story::default();
        s.start_career(0, 234);
        s.crew_team = CrewTeam {
            city: "NEW YORK".into(),
            name: "BALLERS".into(),
            logo: 29,
        };
        s.start_career(1, 235);
        s.crew_team.name = "OTHERS".into();
        let back = Story::read(&s.write()).unwrap();
        assert_eq!(back.crew_team.name, "OTHERS");
        assert_eq!(back.careers[0].as_ref().unwrap().team.city, "NEW YORK");
        assert_eq!(back.careers[0].as_ref().unwrap().team.logo, 29);
        let mut back = back;
        assert!(back.switch_career(0));
        assert_eq!(back.crew_team.name, "BALLERS");
        // A save without the section keeps empty teams.
        let mut plain = Story::default();
        plain.start_career(0, 234);
        assert_eq!(
            Story::read(&plain.write()).unwrap().crew_team,
            CrewTeam::default()
        );
    }

    #[test]
    fn league_challenge_lists_every_original_league_team_including_atlanta_zero() {
        let mut teams: Vec<_> = REGION_MATCHES[..5]
            .iter()
            .flat_map(|teams| teams.iter().copied())
            .collect();
        teams.sort_unstable();
        assert_eq!(teams, (0..29).collect::<Vec<_>>());
        assert_eq!(REGION_MATCHES[4], [19, 13, 12, 17, 0]);
    }

    #[test]
    fn old_rust_nickname_ids_migrate_in_every_career_and_new_rows_round_trip() {
        let mut old = Story::default();
        assert!(old.start_career(0, 234));
        old.nickname = true;
        old.nickname_id = 57; // Old field-0 id, String.
        assert!(old.start_career(1, 235));
        old.nickname = true;
        old.nickname_id = 34; // Old field-0 id, Butta.
        let mut bytes = old.write();
        assert_eq!(&bytes[bytes.len() - 11..bytes.len() - 7], b"NKRW");
        bytes.truncate(bytes.len() - 11);
        let migrated = Story::read(&bytes).unwrap();
        assert_eq!(migrated.nickname_id, 28);
        assert_eq!(migrated.career_at(0).unwrap().nickname_id, 47);
        assert_eq!(Story::read(&migrated.write()).unwrap().nickname_id, 28);
        let row_zero = Story {
            nickname: true,
            nickname_id: 0,
            ..Default::default()
        };
        let back = Story::read(&row_zero.write()).unwrap();
        assert!(back.nickname);
        assert_eq!(back.nickname_id, 0);
    }

    #[test]
    fn star_progress_cannot_demote_or_carry_overflow_into_the_next_tier() {
        let mut s = Story {
            reputation: 12,
            ..Default::default()
        };
        s.play(0, &outcome(false, 0, 0));
        assert_eq!(
            s.reputation, 11,
            "losing clamps to the current tier's floor"
        );
        s.reputation = 10;
        s.play(9, &outcome(true, 0, 0));
        assert_eq!(
            s.reputation, 11,
            "winning discards overflow beyond one tier"
        );
        s.reputation = 55;
        s.play(9, &outcome(true, 0, 0));
        s.play(0, &outcome(false, 0, 0));
        assert_eq!(s.reputation, 55, "the highest tier is terminal");
    }

    #[test]
    fn pending_nickname_is_per_career_persistent_and_legacy_safe() {
        let mut s = Story::default();
        assert!(s.start_career(0, 234));
        s.nickname = true;
        s.nickname_id = 0;
        s.nickname_pending = true;
        assert!(s.start_career(1, 235));
        assert!(!s.nickname_pending);
        let mut back = Story::read(&s.write()).unwrap();
        assert!(!back.nickname_pending);
        assert!(back.switch_career(0));
        assert!(back.nickname_pending);
        back.nickname_pending = false;
        let consumed = Story::read(&back.write()).unwrap();
        assert!(!consumed.nickname_pending);
        assert!(consumed.nickname);
        assert!(!consumed.career_at(1).unwrap().nickname_pending);
        let legacy_bytes = s.write();
        let at = legacy_bytes.windows(4).position(|w| w == b"NKPN").unwrap();
        let mut legacy = Story::read(&legacy_bytes[..at]).unwrap();
        assert!(legacy.switch_career(0));
        assert!(legacy.nickname);
        assert!(!legacy.nickname_pending);
    }

    #[test]
    fn nickname_is_due_only_at_tier_three_even_without_a_new_tier() {
        let mut s = Story {
            reputation: 33,
            ..Default::default()
        };
        let notes = s.play(0, &outcome(true, 0, 0));
        assert!(notes.contains(&Notice::Nickname));
        assert!(s.nickname);
        assert!(s.nickname_pending);
        s.nickname_pending = false;
        assert!(!s.play(0, &outcome(true, 0, 0)).contains(&Notice::Nickname));
        assert!(
            !s.nickname_pending,
            "earning the same nickname does not rearm its call"
        );
        let mut higher = Story {
            reputation: 44,
            ..Default::default()
        };
        assert!(
            !higher
                .play(0, &outcome(true, 0, 0))
                .contains(&Notice::Nickname)
        );
    }

    #[test]
    fn clearing_tournament_admits_the_rival_and_his_team() {
        let mut s = Story::default();
        s.play(4, &outcome(true, 1, 0)); // The Cage pick-up, opens ev 2 clear route
        for _ in 0..(EVENTS[0].target as u32 / 3 + 1) {
            s.play(0, &outcome(true, 1, 0));
        }
        for _ in 0..(EVENTS[1].target as u32 / 4 + 1) {
            s.play(1, &outcome(true, 1, 0));
        }
        // Team Stretch's tournament: three wins to the title.
        s.play(2, &outcome(true, 1, 0));
        s.play(2, &outcome(true, 1, 0));
        let notes = s.play(2, &outcome(true, 1, 0));
        assert!(notes.contains(&Notice::Player(145)), "{notes:?}");
        assert!(notes.contains(&Notice::Team(40)));
        assert!(s.unlocked_players.contains(&145));
        assert!(s.unlocked_teams.contains(&40));
    }

    #[test]
    fn league_challenge_pays_at_the_end() {
        for difficulty in 0..=2 {
            let mut s = Story::default();
            let win = MatchOutcome {
                won: true,
                difficulty,
                ..Default::default()
            };
            for run in 0..2 {
                let before = s.reward_points;
                for (region, teams) in REGION_MATCHES.iter().enumerate() {
                    for index in 0..teams.len() {
                        let notes = s.play_league(region, index, &win);
                        assert!(
                            !notes
                                .iter()
                                .any(|n| matches!(n, Notice::Court(_) | Notice::Team(_)))
                        );
                    }
                    assert!(s.league_boss_open(region));
                    assert!(!s.league_done(region, teams.len()));
                    assert!(!s.complete_league());
                    let notes = s.play_league(region, teams.len(), &win);
                    assert!(notes.contains(&Notice::Court(REGION_REWARDS[region].1)));
                    if run == 0 {
                        assert!(notes.contains(&Notice::Team(REGION_REWARDS[region].0)));
                    }
                }
                assert!(s.complete_league() && s.league_rewarded);
                // 29 league teams plus five regional legends; terminal award once.
                assert_eq!(
                    s.reward_points - before,
                    34 * LEAGUE_RP[usize::from(difficulty)]
                        + LEAGUE_DONE_RP[usize::from(difficulty)]
                );
                let before_repeat = s.reward_points;
                let notes = s.play_league(4, REGION_MATCHES[4].len(), &win);
                assert_eq!(
                    s.reward_points - before_repeat,
                    LEAGUE_RP[usize::from(difficulty)]
                );
                assert!(!notes.contains(&Notice::RewardPoints(
                    LEAGUE_DONE_RP[usize::from(difficulty)]
                )));
                s.owned.push(76);
                s.restart_league();
                assert_eq!(
                    (s.league_played, s.region_cleared, s.league_lost),
                    (0, 0, 0)
                );
                assert!(s.league_rewarded && s.owned.contains(&76));
                assert_eq!(s.unlocked_teams, [31, 32, 33, 34, 35]);
                assert_eq!(s.development_points, 0);
            }
        }
        // A Pick Up win pays by difficulty.
        let mut s = Story::default();
        assert_eq!(s.win_pickup(2), 150);
    }

    #[test]
    fn league_boss_requires_ordinary_wins_and_a_loss_never_clears_it() {
        let mut s = Story {
            games: 17,
            wins: 8,
            losses: 9,
            ..Default::default()
        };
        let win = MatchOutcome {
            won: true,
            ..Default::default()
        };
        let boss = REGION_MATCHES[0].len();
        assert!(s.play_league(0, boss, &win).is_empty());
        assert!(s.play_league(5, 0, &win).is_empty());
        assert!(s.play_league(0, boss + 1, &win).is_empty());
        assert_eq!((s.games, s.wins, s.losses), (17, 8, 9));
        for index in 0..boss {
            s.play_league(0, index, &win);
        }
        assert!(s.league_boss_open(0));
        assert_eq!(s.region_cleared, 0);
        assert!(!s.unlocked_teams.contains(&31));
        let before = s.reward_points;
        s.play_league(0, boss, &MatchOutcome::default());
        assert_eq!(s.reward_points, before);
        assert!(!s.league_done(0, boss));
        let notes = s.play_league(0, boss, &win);
        assert!(s.league_done(0, boss));
        assert!(notes.contains(&Notice::Court(7)) && notes.contains(&Notice::Team(31)));
        assert!(!notes.contains(&Notice::Milestone(5)));
        assert_eq!((s.games, s.wins, s.losses), (17, 8, 9));
    }

    #[test]
    fn legacy_league_saves_keep_earned_rewards_but_incomplete_runs_need_bosses() {
        let without_marker = |s: &Story| {
            let mut bytes = s.write();
            let at = bytes.windows(4).position(|w| w == LEAGUE_BOSSES).unwrap();
            bytes.drain(at..at + 9);
            bytes
        };
        let mut legacy = Story {
            region_cleared: 1,
            league_played: 0x3f,
            reward_points: 1234,
            unlocked_teams: vec![31],
            owned: vec![76],
            milestones: 1 << 5,
            ..Default::default()
        };
        let restored = Story::read(&without_marker(&legacy)).unwrap();
        assert!(restored.league_boss_open(0) && !restored.league_done(0, 6));
        assert_eq!(restored.reward_points, 1234);
        assert_eq!(
            (
                &restored.owned,
                &restored.unlocked_teams,
                restored.milestones
            ),
            (&legacy.owned, &legacy.unlocked_teams, legacy.milestones)
        );
        legacy.league_rewarded = true;
        legacy.league_played |= 1 << 48;
        let restored = Story::read(&without_marker(&legacy)).unwrap();
        assert!(restored.complete_league() && restored.league_rewarded);
        assert_eq!(restored.league_played >> 40, 0);
        assert_eq!(Story::read(&restored.write()).unwrap(), restored);
        let restored = Story::read(&legacy.write()).unwrap();
        assert_eq!(
            restored.region_cleared, 1,
            "new save flags keep their meaning"
        );
    }

    #[test]
    fn a_baller_joins_one_team_and_takes_its_place() {
        let mut s = Story::default();
        s.join_team(5, 245);
        s.join_team(7, 246);
        assert_eq!(s.sixth_man(5), Some(245));
        // Joining another team leaves the first; a team takes one.
        s.join_team(9, 245);
        assert_eq!(s.sixth_man(5), None);
        assert_eq!(s.sixth_man(9), Some(245));
        s.join_team(9, 246);
        assert_eq!(s.sixth_men, [(9, 246)]);
        let back = Story::read(&s.write()).unwrap();
        assert_eq!(back.sixth_men, [(9, 246)]);
        s.leave_team(246);
        assert!(s.sixth_men.is_empty());
    }

    #[test]
    fn the_save_round_trips() {
        let mut s = Story::default();
        s.play(0, &outcome(true, 900_000, 1));
        s.play(2, &outcome(true, 40_000, 2));
        s.play_league(3, 0, &outcome(true, 5, 0));
        s.unlocked_players = vec![145, 158];
        s.unlocked_teams = vec![40];
        s.owned = vec![3, 17];
        s.school = 0b11;
        let bytes = s.write();
        let back = Story::read(&bytes).expect("reads");
        assert_eq!(back, s);
        assert!(
            Story::read(&bytes[..bytes.len() - 2]).is_none(),
            "short buffer"
        );
        assert!(Story::read(b"NOPE").is_none());
    }

    #[test]
    fn ballers_survive_a_save_and_old_saves_still_load() {
        let mut s = Story::default();
        let old = s.write();
        s.ballers.push(CreatedBaller {
            slot: 2,
            first_name: "JAY".into(),
            last_name: "STREETS".into(),
            build: 1,
            height_in: 77,
            weight_lb: 205,
            hair_style: -1,
            facial_hair_style: 3,
            skills: [60, 70, 50, 40, 55, 65, 45],
            ..Default::default()
        });
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.ballers, s.ballers);
        assert_eq!(back.ballers[0].record(), 236);
        // With moves bought, and a save from before moves still reading.
        let no_moves = s.write();
        s.ballers[0].moves = [vec![7], vec![10, 26, 49], vec![]];
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.ballers, s.ballers);
        assert!(
            Story::read(&no_moves).expect("older").ballers[0]
                .moves
                .iter()
                .all(Vec::is_empty)
        );
        // A save written before ballers existed reads with none.
        assert!(Story::read(&old).expect("old save").ballers.is_empty());
        // The icon (field 14) rides in its own section, past the others; a
        // save without it reads every icon as the first.
        let no_icons = s.write();
        s.ballers[0].icon = 5;
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.ballers[0].icon, 5);
        assert_eq!(back.ballers, s.ballers);
        assert_eq!(Story::read(&no_icons).expect("older").ballers[0].icon, 0);
    }

    #[test]
    fn a_damaged_save_is_cut_back_to_the_disc() {
        let mut s = Story {
            crew: vec![10, 9999],
            unlocked_players: vec![150, 400],
            unlocked_teams: vec![29, 300],
            ..Default::default()
        };
        s.ballers.push(CreatedBaller {
            slot: 1,
            moves: [vec![3, 200], vec![], vec![]],
            ..Default::default()
        });
        s.ballers.push(CreatedBaller {
            slot: 1,
            ..Default::default()
        });
        s.ballers.push(CreatedBaller {
            slot: STAR_SLOTS as u8,
            ..Default::default()
        });
        s.teams.push(CreatedTeam {
            roster: [1, 2, 3, 4, 5000],
            ..Default::default()
        });
        assert_eq!(s.sanitize(240, 49, 93), 7);
        assert_eq!(s.crew, vec![10]);
        assert_eq!(s.unlocked_players, vec![150]);
        assert_eq!(s.unlocked_teams, vec![29]);
        assert_eq!(s.ballers.len(), 1);
        assert_eq!(s.ballers[0].moves[0], vec![3, EMPTY_MOVE_SLOT]);
        assert!(s.teams.is_empty());
        // A sound save loses nothing.
        assert_eq!(s.sanitize(240, 49, 93), 0);
    }

    #[test]
    fn milestones_come_from_the_modes() {
        let win = |won| MatchOutcome {
            won,
            difficulty: 1,
            ..Default::default()
        };
        // A region without a loss: milestone 6 (index 5).
        let mut s = Story::default();
        let mut got = Vec::new();
        for k in 0..=REGION_MATCHES[0].len() {
            got.extend(s.play_league(0, k, &win(true)));
        }
        assert!(got.contains(&Notice::Milestone(5)));
        // A loss on the way spoils it.
        let mut s = Story::default();
        s.play_league(1, 0, &win(false));
        let mut got = Vec::new();
        for k in 0..=REGION_MATCHES[1].len() {
            got.extend(s.play_league(1, k, &win(true)));
        }
        assert!(!got.contains(&Notice::Milestone(6)));
        // Reputation level 1 is milestone 15 (index 14).
        let mut s = Story::default();
        let mut got = Vec::new();
        for _ in 0..12 {
            got.extend(s.play(9, &win(true)));
        }
        assert!(s.reputation_level() >= 1);
        assert!(got.contains(&Notice::Milestone(14)), "{got:?}");
        // Street School, clean: both school milestones; they ride the save.
        s.school = (1 << SCHOOL_LESSONS) - 1;
        assert_eq!(
            s.school_milestones(),
            vec![Notice::Milestone(10), Notice::Milestone(11)]
        );
        assert!(s.school_milestones().is_empty());
        s.modes_played = 0b101;
        let back = Story::read(&s.write()).expect("reads");
        assert_eq!((back.milestones, back.modes_played), (s.milestones, 0b101));
    }

    #[test]
    fn careers_keep_their_own_progress() {
        let win = MatchOutcome {
            won: true,
            ..Default::default()
        };
        let mut s = Story::default();
        assert!(s.start_career(0, 234));
        s.crew.extend([184, 185]);
        s.play(0, &win);
        let first = s.progress[0];
        assert!(first > 0);
        // A second career starts from nothing; the first keeps its own.
        assert!(s.start_career(3, 235));
        assert_eq!((s.progress[0], s.crew.clone()), (0, vec![235]));
        assert_eq!(s.career_at(0).unwrap().progress[0], first);
        // Both ride in the save, the one being played included.
        s.reputation = 7;
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.active, Some(3));
        assert_eq!(back.reputation, 7);
        assert_eq!(back.career_at(0).unwrap().progress[0], first);
        let mut back = back;
        assert!(back.switch_career(0));
        assert_eq!((back.progress[0], back.crew[0]), (first, 234));
        assert_eq!(back.career_at(3).unwrap().reputation, 7);
        back.delete_career(0);
        assert!(back.active.is_none() && back.crew.is_empty());
        // A save from before careers becomes career 1.
        let old = Story {
            crew: vec![150, 184, 185],
            ..Default::default()
        };
        let mut bytes = old.write();
        // Without its CARS section (an older build wrote none).
        if let Some(at) = bytes.windows(4).position(|w| w == b"CARS") {
            bytes.truncate(at);
        }
        let migrated = Story::read(&bytes).expect("reads");
        assert_eq!(migrated.active, Some(0));
        assert_eq!(migrated.career_at(0).unwrap().crew, vec![150, 184, 185]);
    }

    #[test]
    fn native_move_holes_survive_assignment_save_and_pruning() {
        let mut b = CreatedBaller::default();
        assert!(b.assign_in_slot(2, 10, 69));
        assert_eq!(b.moves[2][5], EMPTY_MOVE_SLOT);
        assert_eq!(b.moves[2][10], 69);
        assert!(!b.assign_in_slot(2, 5, 69));
        assert!(b.assign_in_slot(2, 10, 70));
        assert!(b.owns(69));
        b.moves[2][7] = 100;
        let mut story = Story::default();
        story.ballers.push(b);
        assert_eq!(story.sanitize(400, 100, 93), 1);
        assert_eq!(story.ballers[0].moves[2][7], EMPTY_MOVE_SLOT);
        assert_eq!(story.ballers[0].moves[2][10], 70);
        let restored = Story::read(&story.write()).unwrap();
        assert_eq!(restored.ballers[0].moves, story.ballers[0].moves);
    }

    #[test]
    fn moves_stay_unlocked_when_replaced() {
        let mut b = CreatedBaller {
            slot: 2,
            ..Default::default()
        };
        assert!(b.assign(1, None, 10));
        assert!(b.assign(1, Some(10), 11)); // 11 replaces 10
        assert_eq!(b.moves[1], vec![11]);
        assert!(b.owns(10) && b.owns(11)); // 10 comes back for free
        assert!(!b.assign(1, None, 11)); // already in a slot
        for m in 20..30 {
            assert!(b.assign(1, None, m));
        }
        assert!(!b.assign(1, None, 40)); // eleven dunk slots
        let mut s = Story::default();
        s.ballers.push(b.clone());
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.ballers[0].unlocked, b.unlocked);
        assert_eq!(back.ballers[0].moves, b.moves);
    }

    #[test]
    fn trophies_follow_the_modes() {
        let mut s = Story::default();
        assert_eq!(s.trophies(), [false; 4]);
        s.school = (1 << SCHOOL_LESSONS) - 2;
        assert!(!s.trophies()[0]);
        s.school |= 1;
        assert_eq!(s.trophies(), [true, false, false, false]);
        s.league_rewarded = true;
        s.tournament_cleared = !0;
        s.street_cleared = !0;
        s.progress = [u8::MAX; EVENTS.len()];
        assert_eq!(s.trophies(), [true, true, true, false]);
        s.reputation = REP_LEVEL_SIZE * i16::from(REP_MAX_LEVEL);
        assert_eq!(s.trophies(), [true; 4]);
        s.reward_points = 250;
        assert_eq!(
            s.trophy_rewards(),
            [Notice::Jersey(34), Notice::BossMove(44), Notice::Card(34)]
        );
        assert!(s.trophy_rewards().is_empty());
        assert_eq!(s.reward_points, 250);
        // Move 44 is the boss-move slot's, not court card 44.
        assert!(s.owned.contains(&34) && !s.owned.contains(&44));
        assert!(s.unlocked_players.contains(&167));
        assert!(s.unlocked_teams.contains(&34));
    }

    #[test]
    fn the_rucker_classics_card_switches_its_song_on() {
        let bit = 1 << crate::records::RUCKER_CLASSIC_SONG_BIT;
        let mut s = Story::default();
        // The reset leaves it off.
        assert_ne!(s.settings.unwrap_or_default().songs_off & bit, 0);
        s.court_granted(7);
        assert_eq!((s.settings, s.songs_switched_on), (None, 0));
        assert!(s.buy(44, 0, CardGrant::Court(10)));
        assert_eq!(s.settings.unwrap().songs_off & bit, 0);
        assert_eq!(s.songs_switched_on, bit);
        // The switch is the live options' to take; it is not saved.
        assert_eq!(Story::read(&s.write()).unwrap().songs_switched_on, 0);
    }

    #[test]
    fn development_points_belong_to_the_baller_across_careers_and_saves() {
        let mut s = Story::default();
        s.start_career(0, 234);
        s.play(0, &outcome(true, 50, 0));
        assert_eq!(s.points_for(234), 175);
        s.start_career(1, 235);
        assert_eq!(s.star_points(), 0);
        s.reward_points = 500;
        assert!(s.spend_reward_points_for(235, 200, 100));
        assert!(!s.spend_baller(235, 101));
        assert!(s.spend_baller(235, 75));
        s.switch_career(0);
        assert_eq!(s.star_points(), 175);
        let back = Story::read(&s.write()).unwrap();
        assert_eq!(back.points_for(234), 175);
        assert_eq!(back.points_for(235), 25);
        assert_eq!(back.reward_points, 300);
    }

    /// The ballers' wardrobes go into the save's last section (`WARD`); a
    /// save written before it owns what each baller wears, which the old
    /// editor had charged for.
    #[test]
    fn the_wardrobe_saves_and_older_saves_own_what_is_worn() {
        let mut b = CreatedBaller {
            slot: 1,
            build: 2,
            clothing: crate::cab::OUTFITS[2][7],
            clothing_variant: 2,
            shoes: 14,
            first_name: "A".into(),
            last_name: "B".into(),
            ..Default::default()
        };
        b.wardrobe.buy_outfit(3, 1, false);
        b.wardrobe.buy_shoes(5);
        let s = Story {
            ballers: vec![b.clone()],
            ..Default::default()
        };
        let data = s.write();
        let back = Story::read(&data).unwrap();
        assert_eq!(back.ballers[0].wardrobe, b.wardrobe);
        let ward = data
            .windows(4)
            .rposition(|w| w == b"WARD")
            .expect("the wardrobe's section");
        assert_eq!(ward + 4 + 4 + 1 + 13, data.len(), "written last");
        let old = Story::read(&data[..ward]).unwrap();
        let w = old.ballers[0].wardrobe;
        assert_eq!(b.outfit_row(), 7);
        assert!(w.owns_outfit(7, 2) && !w.owns_outfit(3, 1));
        assert!(w.owns_shoes(crate::cab::shoe_row(14)) && !w.owns_shoes(5));
    }

    #[test]
    fn an_old_shared_balance_migrates_once_to_the_active_baller() {
        let s = Story {
            crew: vec![234, 184, 185],
            development_points: 650,
            ..Default::default()
        };
        let mut back = Story::read(&s.write()).unwrap();
        assert_eq!(back.development_points, 0);
        assert_eq!(back.points_for(234), 650);
        assert_eq!(back.points_for(235), 0);
        back.credit_baller(235, 100);
        assert_eq!(back.points_for(234), 650);
        assert_eq!(
            Story::read(&back.write()).unwrap().baller_points,
            back.baller_points
        );
    }

    #[test]
    fn school_owners_attempts_and_guest_saves_follow_native_payloads() {
        let mut s = Story {
            school: 3,
            school_failed: true,
            ..Default::default()
        };
        s.records.add_user("A").unwrap();
        s.records.add_user("B").unwrap();
        s.select_school_user(Some(0));
        assert_eq!(s.school, 0);
        s.school_profile.attempts[0] = i32::MAX;
        s.school_profile.challenges[32] = 15;
        assert!(s.start_school_lesson(0));
        assert_eq!(s.school_profile.attempts[0], i32::MIN);
        s.finish_school_lesson(0, false);
        assert_eq!(s.school_profile.challenges[0], 4);
        s.start_school_lesson(0);
        s.finish_school_lesson(0, true);
        assert_eq!(s.school_profile.challenges[0], 12);
        let a = s.school_progress();
        s.start_school_lesson(0);
        s.finish_school_lesson(0, false);
        assert_eq!(
            s.school_profile.challenges[0], 12,
            "completed replay retains its outcome"
        );
        s.select_school_user(Some(1));
        assert_eq!(s.school, 0);
        s.start_school_lesson(1);
        s.finish_school_lesson(1, true);
        let b = s.school_progress();
        s.select_school_user(Some(0));
        assert_eq!(s.school, 1);
        assert_eq!(s.school_profile.challenges, a.challenges);
        s.select_school_user(None);
        s.start_school_lesson(2);
        s.finish_school_lesson(2, true);
        let mut loaded = Story::read(&s.write()).unwrap();
        assert_eq!((loaded.school, loaded.school_failed), (3, true));
        loaded.select_school_user(Some(1));
        assert_eq!(loaded.school_progress(), b);
        loaded.select_school_user(Some(0));
        assert_eq!(loaded.school_profile.challenges[32], 15);
        loaded.select_school_user(None);
        assert_eq!(loaded.school, 0);
        assert_eq!(loaded.school_profile.attempts, [0; 26]);
        assert!(!loaded.start_school_lesson(26));
        assert!(!loaded.finish_school_lesson(26, true));
    }

    #[test]
    fn school_unknown_original_payout_and_legacy_owner_remain_unassigned() {
        let all = (1 << SCHOOL_LESSONS) - 1;
        let mut s = Story {
            school: all,
            school_failed: true,
            reward_points: 333,
            ..Default::default()
        };
        s.records.add_user("LEGACY OWNER UNKNOWN").unwrap();
        s.select_school_user(Some(0));
        assert_eq!(s.school, 0);
        assert!(s.school_completion_rewards().is_empty());
        assert!(s.school_trophy);
        assert_eq!(s.reward_points, 333);
        s.select_school_user(None);
        // Install the unmapped original payload after detaching the live cache.
        let profile = &mut s.records.users[0].school;
        profile.challenges[..26].fill(8);
        profile.payout_known = false;
        let mut loaded = Story::read(&s.write()).unwrap();
        assert_eq!((loaded.school, loaded.school_failed), (all, true));
        loaded.select_school_user(Some(0));
        assert!(!loaded.school_profile.payout_known);
        assert_eq!(loaded.school, all);
        assert!(loaded.school_completion_rewards().is_empty());
        assert_eq!(loaded.reward_points, 333);
    }

    #[test]
    fn school_payout_is_once_per_user_and_trophy_is_shared() {
        let mut s = Story::default();
        s.records.add_user("A").unwrap();
        s.records.add_user("B").unwrap();
        for user in 0..2 {
            s.select_school_user(Some(user));
            if user == 0 {
                s.finish_school_lesson(0, false);
            }
            for lesson in 0..26 {
                s.start_school_lesson(lesson);
                s.finish_school_lesson(lesson, true);
            }
            let rewards = s.school_completion_rewards();
            assert!(rewards.contains(&Notice::RewardPoints(1000)));
            assert_eq!(rewards.contains(&Notice::Milestone(11)), user == 1);
            assert!(s.school_completion_rewards().is_empty());
            assert!(s.trophies()[0]);
        }
        assert_eq!(s.reward_points, 2000);
        let mut loaded = Story::read(&s.write()).unwrap();
        loaded.select_school_user(Some(0));
        assert!(loaded.school_completion_rewards().is_empty());
        loaded.select_school_user(None);
        assert_eq!(loaded.school, 0);
        assert!(
            loaded.trophies()[0],
            "new guests retain the shared lifetime trophy"
        );
        assert_eq!(loaded.reward_points, 2000);
    }

    #[test]
    fn league_profiles_keep_owners_and_preserve_unassigned_legacy_progress() {
        let mut s = Story {
            league_played: 0x12,
            league_lost: 4,
            ..Default::default()
        };
        s.select_league_roster(&[1, 2, 3]);
        let legacy = s.league_progress();
        s.records.add_user("A").unwrap();
        s.records.add_user("B").unwrap();
        s.select_league_user(Some(0));
        assert_eq!(s.league_played, 0);
        s.start_league_region(0);
        s.select_league_roster(&[4, 5, 6]);
        s.play_league(0, 0, &outcome(true, 0, 0));
        let a = s.league_progress();
        s.select_league_user(Some(1));
        assert_eq!(s.league_played, 0);
        s.start_league_region(1);
        s.play_league(1, 0, &outcome(false, 0, 0));
        let b = s.league_progress();
        s.select_league_user(Some(0));
        assert_eq!(s.league_progress(), a);
        s.select_league_user(None);
        s.select_league_roster(&[7, 8, 9]);
        s.play_league(4, 0, &outcome(true, 0, 0));
        assert_ne!(s.league_played, 0);
        let mut loaded = Story::read(&s.write()).unwrap();
        assert_eq!(loaded.league_progress(), legacy);
        assert_eq!(loaded.records.users[0].league, a);
        assert_eq!(loaded.records.users[1].league, b);
        loaded.select_league_user(None);
        assert_eq!(loaded.league_played, 0);
        assert!(loaded.league_roster.is_empty());
        loaded.select_league_user(Some(0));
        assert_eq!(loaded.league_progress(), a);
        // Names own payloads even when deleting an earlier vector entry.
        loaded.records.delete_user(0);
        loaded.select_league_user(Some(0));
        assert_eq!(loaded.league_progress(), b);
        assert_eq!(loaded.records.users[0].name, "B");
    }

    #[test]
    fn league_own_team_commit_survives_team_changes_and_reload() {
        for won in [false, true] {
            let mut s = Story::default();
            s.records.add_user("A").unwrap();
            s.select_league_user(Some(0));
            s.league_team = Some(0); // Atlanta is the fifth Southeast opponent.
            s.start_league_region(4);
            s.play_league(4, 0, &outcome(won, 0, 0));
            assert_ne!(s.league_played & league_bit(4, 4), 0);
            s.league_team = Some(1);
            assert!(s.league_done(4, 4));
            let mut loaded = Story::read(&s.write()).unwrap();
            loaded.select_league_user(Some(0));
            assert_eq!(loaded.league_team, Some(1));
            assert!(loaded.league_done(4, 4));
        }
        for won in [false, true] {
            let mut s = Story::default();
            s.records.add_user("A").unwrap();
            s.select_league_user(Some(0));
            s.league_team = Some(0);
            for index in 0..4 {
                s.league_played |= league_bit(4, index);
            }
            s.play_league(4, 5, &outcome(won, 0, 0));
            s.league_team = Some(1);
            assert_eq!(
                s.league_done(4, 4),
                won,
                "boss losses never commit the own-team bit"
            );
            let mut loaded = Story::read(&s.write()).unwrap();
            loaded.select_league_user(Some(0));
            assert_eq!(loaded.league_done(4, 4), won);
        }
    }

    #[test]
    fn league_native_own_team_counts_without_manufacturing_an_ordinary_win() {
        let mut s = Story::default();
        s.records.add_user("A").unwrap();
        s.select_league_user(Some(0));
        s.league_team = Some(REGION_MATCHES[4][4]); // Atlanta, valid team ID zero.
        assert!(s.league_done(4, 4));
        assert_eq!(s.league_played, 0);
        for index in 0..4 {
            s.play_league(4, index, &outcome(true, 0, 0));
        }
        assert!(s.league_boss_open(4));
        assert!(!s.league_done(4, 5));
        s.play_league(4, 5, &outcome(true, 0, 0));
        assert!(s.league_done(4, 5));
    }

    #[test]
    fn league_selection_is_separate_and_rejects_invalid_teams() {
        let mut s = Story {
            crew: vec![234, 184, 185],
            ..Default::default()
        };
        assert!(!s.select_league_roster(&[1, 2]));
        assert!(!s.select_league_roster(&[1, 1, 2]));
        assert!(s.select_league_roster(&[1, 2, 3]));
        let back = Story::read(&s.write()).unwrap();
        assert_eq!(back.league_roster, [1, 2, 3]);
        assert_eq!(back.crew, [234, 184, 185]);
        s.league_roster = vec![1, 2, 9999];
        s.sanitize(240, 49, 93);
        assert!(s.league_roster.is_empty());
    }

    #[test]
    fn an_league_loss_beats_nobody() {
        let mut s = Story::default();
        s.play_league(0, 0, &MatchOutcome::default());
        assert!(!s.league_done(0, 0));
        s.play_league(
            0,
            0,
            &MatchOutcome {
                won: true,
                ..Default::default()
            },
        );
        assert!(s.league_done(0, 0));
    }

    #[test]
    fn the_map_holds_every_event_once() {
        let mut seen: Vec<usize> = (0..CITIES.len()).flat_map(city_events).collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..EVENTS.len()).collect::<Vec<_>>());
        // Every event sits in its court's city.
        assert_eq!(city_of(9), 0); // Rucker Park, Harlem
        assert_eq!(city_of(1), 1); // Broad Street, Philadelphia
        assert_eq!(city_of(7), 5); // Lincoln College, Los Angeles
        assert_eq!(kind_word(9).1, "Street Championship");
        assert_eq!(kind_word(10).1, "Legendary Challenge");
        // Clearing the first pick-up's target opens what it gates.
        let mut s = Story::default();
        let win = MatchOutcome {
            won: true,
            ..Default::default()
        };
        let mut opened = Vec::new();
        for _ in 0..5 {
            for n in s.play(0, &win) {
                if let Notice::EventOpen(e) = n {
                    opened.push(e);
                }
            }
        }
        assert!(!opened.is_empty());
        assert!(opened.iter().all(|&e| s.open(usize::from(e))));
    }

    #[test]
    fn update_team_follows_the_game() {
        let win = MatchOutcome {
            won: true,
            ..Default::default()
        };
        let mut s = Story {
            crew: vec![150, 184, 185],
            ..Default::default()
        };
        // A pick-up won against players not on the team: offered.
        assert!(s.offers_update(0, true, &[190, 191, 192]));
        assert!(!s.offers_update(0, false, &[190, 191, 192]));
        // All of them on the team already: not offered.
        assert!(!s.offers_update(0, true, &[150, 184, 185]));
        // The fourth and fifth fill the open positions, then they replace.
        assert!(s.update_team(4, 190));
        assert_eq!(s.crew, vec![150, 184, 185, 190]);
        assert!(s.update_team(4, 191));
        assert_eq!(s.crew, vec![150, 184, 185, 190, 191]);
        assert!(s.update_team(1, 192));
        assert_eq!(s.crew, vec![150, 192, 185, 190, 191]);
        // The star's position, a player already on the team: refused.
        assert!(!s.update_team(0, 193));
        assert!(!s.update_team(2, 191));
        // A starter to the bench and back.
        s.toggle_starter(4);
        assert_eq!(s.crew, vec![150, 192, 191, 190, 185]);
        s.toggle_starter(1);
        assert_eq!(&s.crew[..3], &[150, 191, 190]);
        assert!(s.crew.contains(&192));
        // A tournament: only once its title is won.
        let mut t = Story {
            crew: vec![150, 184, 185],
            ..Default::default()
        };
        t.play(2, &win);
        assert!(!t.offers_update(2, true, &[190]));
        t.play(2, &win);
        t.play(2, &win);
        assert!(t.cleared(2));
        assert!(t.offers_update(2, true, &[150]));
    }

    #[test]
    fn a_tournament_takes_the_whole_bracket() {
        let mut s = Story::default();
        let win = MatchOutcome {
            won: true,
            ..Default::default()
        };
        // Event 2: eight crews, single elimination, Stretch's crew (40).
        assert_eq!(
            s.tournament(2).rows[3].sides[0],
            crate::bracket::Entrant::Boss(40)
        );
        let notes = s.play(2, &win);
        assert!(notes.contains(&Notice::NextRound { kind: 0, round: 1 }));
        assert!(!s.cleared(2));
        s.play(2, &win);
        s.play(2, &win);
        assert!(s.cleared(2));
        // The bracket rides in the save.
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.brackets, s.brackets);
        // A loss in a fresh one knocks you out; the next starts over.
        let mut s = Story::default();
        let notes = s.play(2, &MatchOutcome::default());
        assert!(notes.contains(&Notice::Eliminated));
        assert_eq!(s.tournament(2).current().map(|c| c.0), Some(0));
    }

    #[test]
    fn records_and_options_ride_in_the_save() {
        let mut s = Story::default();
        s.records.add_user("JAY").unwrap();
        s.settings = Some(crate::records::Settings {
            music: 2,
            ..Default::default()
        });
        s.ballers.push(CreatedBaller {
            slot: 0,
            moves: [vec![1], vec![], vec![]],
            ..Default::default()
        });
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.records, s.records);
        assert_eq!(back.settings, s.settings);
        assert_eq!(back.ballers, s.ballers);
    }

    #[test]
    fn created_teams_survive_a_save() {
        let mut s = Story::default();
        s.ballers.push(CreatedBaller {
            slot: 1,
            first_name: "JAY".into(),
            ..Default::default()
        });
        s.teams.push(CreatedTeam {
            city: "New York".into(),
            name: "Ballers".into(),
            logo: 41,
            roster: [55, 56, 158, 145, 235],
        });
        let back = Story::read(&s.write()).expect("reads back");
        assert_eq!(back.teams, s.teams);
        assert_eq!(back.ballers, s.ballers);
        // A name longer than a length byte is cut at a character boundary.
        let mut long = Story::default();
        long.teams.push(CreatedTeam {
            city: "é".repeat(200),
            ..Default::default()
        });
        let back = Story::read(&long.write()).expect("reads back");
        assert_eq!(back.teams[0].city, "é".repeat(127));
    }

    #[test]
    fn the_shop_takes_points_and_grants_legends() {
        let mut s = Story {
            reward_points: 1000,
            ..Default::default()
        };
        // The shop maps category-1 index k to player 145 + k; card 16 is
        // category 1, index 7, so it grants player 152.
        assert!(s.buy(16, 1000, CardGrant::Player(152)));
        assert_eq!(s.reward_points, 0);
        assert!(s.owned.contains(&16));
        assert!(s.unlocked_players.contains(&152));
        assert!(s.unlocked_teams.contains(&29), "the street legends join");
        // Twice does nothing (already owned, and broke).
        assert!(!s.buy(16, 1000, CardGrant::Player(152)));
        // Out of points: refused without taking anything.
        s.reward_points = 100;
        assert!(!s.buy(17, 1000, CardGrant::Player(153)));
        assert_eq!(s.owned.iter().filter(|&&c| c == 17).count(), 0);
    }
}
