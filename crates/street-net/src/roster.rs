//! The anti-cheat for edited rosters: what a match setup's roster and world
//! must satisfy before a server runs it ([`check_setup`]), and what a joiner
//! re-checks on what it is sent.
//!
//! A setup's `roster_data` is a packed [`MatchRoster`]
//! (`street_assets::roster_share`): the creator's roster as differences from
//! the disc, and the line-up's six records whole. The checks:
//!
//! 1. The roster obeys `street_assets::roster_rules` (sizes, ratings, the
//!    rating-total cap, Create A Baller's limits, moves, the fields a roster
//!    may change, counts, team lists). Without the disc that is what the
//!    differences and the line-up's records show; with it
//!    ([`DiscTables`]), every changed record is rebuilt and checked whole,
//!    and the line-up must be the rebuilt roster's own records.
//! 2. The world the creator built holds exactly the line-up's values: each
//!    player's ratings (the record's / 99), reach (from the height), gait
//!    profile (weight, sex, record, handles), staged model scale and width
//!    ratio (the record's model height and weight, which a staged
//!    gamebreaker's actors and hands are placed by) and equipped dribble
//!    moves. So a modified client cannot declare a legal roster and play
//!    other numbers. The one thing allowed to differ is what the cheat codes
//!    set for every player alike (speed, long range, the tiny players' half
//!    scale). Online matches play the records as
//!    they are: the game leaves the story crew's Gym development out of an
//!    online world (`street-game`'s `online.rs`).
//!
//! A setup without roster data plays the disc's roster: its records must be
//! the disc's (with the disc, the world is checked against them; without,
//! each world player must still be a legal player).
//!
//! A roster built on a roster mod names the mod
//! (`street_assets::db::roster_mod_id`). A server accepts it only if it
//! holds that mod ([`Known::mods`], `street-server --roster-db`), against
//! whose own records it checks the rating-total cap (a record no better than
//! its base record may stand above it: Legendary's Gianna Bryant); else it
//! refuses with [`RosterError::UnknownMod`], unless it trusts mods it lacks
//! ([`Known::trust_mods`], `--trust-roster-mods`), when a record over the
//! cap is taken to be the mod's own.

use street_assets::db::{Player, Team};
use street_assets::roster_edits::EMPTY;
use street_assets::roster_rules::{self, Limits};
use street_assets::roster_share::{Base, DISC_RECORDS, MatchRoster};
use street_sim::World;
use street_sim::gait_select::GaitProfile;
use street_sim::player::reach_for_height;

use crate::protocol::MatchSetup;

/// The disc's own player and team tables, for a server that has the disc
/// (`street-server --vol2`, or a game hosting).
#[derive(Clone, Debug, Default)]
pub struct DiscTables {
    pub players: Vec<Player>,
    pub teams: Vec<Team>,
}

impl DiscTables {
    fn pair(&self) -> (&[Player], &[Team]) {
        (&self.players, &self.teams)
    }
}

/// A roster mod a server accepts: its id and tables.
#[derive(Clone, Debug)]
pub struct ModTables {
    pub id: [u8; 32],
    pub players: Vec<Player>,
    pub teams: Vec<Team>,
}

/// What a checker knows: the disc's tables, the roster mods it accepts, and
/// whether it trusts a mod it does not hold.
#[derive(Clone, Debug, Default)]
pub struct Known {
    pub disc: Option<DiscTables>,
    pub mods: Vec<ModTables>,
    pub trust_mods: bool,
}

/// Why a setup is refused.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RosterError {
    /// Built on a roster mod this checker does not hold (its id).
    UnknownMod([u8; 32]),
    /// The roster or the world breaks the rules.
    Illegal(Vec<String>),
}

impl RosterError {
    /// The reasons, said plainly.
    pub fn reasons(&self) -> Vec<String> {
        match self {
            Self::UnknownMod(id) => vec![format!(
                "this server doesn't have the {} roster mod",
                street_assets::roster_share::hex(id)
            )],
            Self::Illegal(r) => r.clone(),
        }
    }
}

/// The disc's teams.
const DISC_TEAMS: usize = 49;
/// The world's ratings and the record's they come from (`setup.rs`'s
/// `ratings`): power, and ratings 9 and 10, are not in the world.
const WORLD_RATINGS: [(&str, usize); 8] = [
    ("shots", 0),
    ("threes", 1),
    ("dunks", 2),
    ("rebounds", 3),
    ("blocks", 5),
    ("steals", 6),
    ("handles", 7),
    ("speed", 8),
];

fn world_rating(p: &street_sim::Player, rating: usize) -> f32 {
    let r = &p.ratings;
    match rating {
        0 => r.shots,
        1 => r.threes,
        2 => r.dunks,
        3 => r.rebounds,
        5 => r.blocks,
        6 => r.steals,
        7 => r.handles,
        _ => r.speed,
    }
}

/// Check a match setup's roster and world. Returns the unpacked roster (for
/// a joiner to play), or every reason the setup is illegal.
pub fn check_setup(s: &MatchSetup, known: &Known) -> Result<Option<MatchRoster>, RosterError> {
    let disc = known.disc.as_ref();
    let records: Vec<usize> = s.roster.iter().map(|&r| usize::from(r)).collect();
    let mut problems = Vec::new();
    if s.roster_data.is_empty() {
        if records.iter().any(|&r| r >= DISC_RECORDS)
            || s.teams.iter().any(|&t| usize::from(t) >= DISC_TEAMS)
        {
            problems.push("the match names players or teams past the disc's own".to_string());
        }
        let limits = Limits::for_base(Base::Original);
        match disc {
            Some(d) => {
                let lineup: Vec<Option<&Player>> = records
                    .iter()
                    .map(|&r| {
                        // Created ballers are the creator's own (not on the disc).
                        (!roster_rules::is_created(r))
                            .then(|| d.players.get(r))
                            .flatten()
                    })
                    .collect();
                problems.extend(check_world(&s.world, &lineup, &records, &limits));
            }
            None => problems.extend(check_world(
                &s.world,
                &vec![None; records.len()],
                &records,
                &limits,
            )),
        }
        return if problems.is_empty() {
            Ok(None)
        } else {
            Err(RosterError::Illegal(problems))
        };
    }
    let m = MatchRoster::unpack(&s.roster_data).map_err(|e| RosterError::Illegal(vec![e]))?;
    let ctx = match m.roster.base {
        Base::Original => roster_rules::Context {
            disc: disc.map(DiscTables::pair),
            base: disc.map(|d| d.players.as_slice()),
            limits: Limits::for_base(Base::Original),
        },
        Base::Mod(id) => match known.mods.iter().find(|t| t.id == id) {
            Some(t) => roster_rules::Context {
                disc: disc.map(DiscTables::pair),
                base: Some(&t.players),
                limits: Limits::from_players(&t.players),
            },
            None if known.trust_mods => roster_rules::Context {
                disc: disc.map(DiscTables::pair),
                base: None,
                limits: Limits {
                    trusted: true,
                    ..Limits::for_base(m.roster.base)
                },
            },
            None => return Err(RosterError::UnknownMod(id)),
        },
    };
    problems.extend(
        roster_rules::check_match(&m, &records, &ctx)
            .iter()
            .map(ToString::to_string),
    );
    if s.teams
        .iter()
        .any(|&t| usize::from(t) >= usize::from(m.roster.teams_len))
    {
        problems.push("the match names a team past the roster's".into());
    }
    if m.lineup.len() == records.len() {
        let lineup: Vec<Option<&Player>> = m.lineup.iter().map(Some).collect();
        problems.extend(check_world(&s.world, &lineup, &records, &ctx.limits));
        problems.extend(check_tricks(&s.world, &m, &records));
    }
    if problems.is_empty() {
        Ok(Some(m))
    } else {
        Err(RosterError::Illegal(problems))
    }
}

/// The world against the line-up's records (`None`: a record not known
/// here, which must still make a legal player).
fn check_world(
    world: &World,
    lineup: &[Option<&Player>],
    records: &[usize],
    limits: &Limits,
) -> Vec<String> {
    let mut out = Vec::new();
    if world.players.len() != lineup.len() {
        out.push(format!(
            "the world has {} players, the line-up {}",
            world.players.len(),
            lineup.len()
        ));
        return out;
    }
    // Cheat codes set a rating for everyone alike.
    let all = |f: &dyn Fn(&street_sim::Player) -> bool| world.players.iter().all(f);
    let speed_cheat = all(&|p| p.ratings.speed == 1.0);
    let threes_cheat = all(&|p| p.ratings.threes == 0.0) || all(&|p| p.ratings.threes == 1.0);
    let reach = [
        reach_for_height(f32::from(roster_rules::HEIGHT_IN[0])),
        reach_for_height(f32::from(roster_rules::HEIGHT_IN[1])),
    ];
    for (k, (p, (record, &number))) in world
        .players
        .iter()
        .zip(lineup.iter().zip(records))
        .enumerate()
    {
        let who = format!("world player {k} (record {number})");
        let created = roster_rules::is_created(number);
        let cap = if created {
            roster_rules::CAB_TOTAL_CAP
        } else {
            limits.total_cap
        };
        let cheated = |rating: usize| rating == 8 && speed_cheat || rating == 1 && threes_cheat;
        let Some(r) = record else {
            // Unknown record: a legal player all the same.
            let total: i32 = WORLD_RATINGS
                .iter()
                .filter(|&&(_, k)| !cheated(k))
                .map(|&(_, k)| (world_rating(p, k) * 99.0).round() as i32)
                .sum();
            if total > cap {
                out.push(format!(
                    "{who}: ratings total {total}, over the cap of {cap}"
                ));
            }
            if !(reach[0] - 1e-4..=reach[1] + 1e-4).contains(&p.ratings.reach) {
                out.push(format!(
                    "{who}: reach {} is outside a {}..{} in player's",
                    p.ratings.reach,
                    roster_rules::HEIGHT_IN[0],
                    roster_rules::HEIGHT_IN[1]
                ));
            }
            continue;
        };
        for &(name, rating) in &WORLD_RATINGS {
            if cheated(rating) {
                continue;
            }
            let value = world_rating(p, rating);
            let own = r.ratings[rating];
            let expect = (f32::from(own) / 99.0).clamp(0.0, 1.0);
            if (value - expect).abs() > 1e-5 {
                out.push(format!(
                    "{who}: {name} {:.1} in the world, the record's {own}",
                    value * 99.0
                ));
            }
        }
        let expect = reach_for_height(f32::from(r.height_in));
        if (p.ratings.reach - expect).abs() > 1e-4 {
            out.push(format!(
                "{who}: reach {} in the world, a {} in player's is {expect}",
                p.ratings.reach, r.height_in
            ));
        }
        if let Some(profile) = world.gait.as_ref().and_then(|g| g.profiles.get(k)) {
            let want = GaitProfile {
                category: GaitProfile::category_for(i32::from(r.weight_lb), r.female),
                identity: number as i32,
                handles: (p.ratings.handles * 99.0).round() as i32,
            };
            if *profile != want {
                out.push(format!(
                    "{who}: gait profile {profile:?}, the record's {want:?}"
                ));
            }
        }
    }
    if let Some(g) = &world.gait
        && g.profiles.len() != world.players.len()
    {
        out.push(format!(
            "{} gait profiles for {} players",
            g.profiles.len(),
            world.players.len()
        ));
    }
    out.extend(check_staged(world, lineup, records));
    out
}

/// The staged players' model scales and width ratios against the records:
/// the model's height scale (`0x3153d0`, [`Player::height_scale`]) and its
/// width over its height ([`street_assets::clip::native_body_width_ratio`]),
/// as `street-game`'s setup makes them; the scale halved for every player
/// alike by the tiny players' code (`FUN_003153d0`), the ratio not. A
/// record not known here must still have a legal model's.
fn check_staged(world: &World, lineup: &[Option<&Player>], records: &[usize]) -> Vec<String> {
    let mut out = Vec::new();
    let Some(staged) = &world.staged else {
        return out;
    };
    let (scales, ratios) = (&staged.player_scales, &staged.player_width_ratios);
    if scales.len() != world.players.len() || ratios.len() != world.players.len() {
        out.push(format!(
            "{} staged scales and {} width ratios for {} players",
            scales.len(),
            ratios.len(),
            world.players.len()
        ));
        return out;
    }
    // The size code, read off the first known record.
    let factor = lineup
        .iter()
        .zip(scales)
        .find_map(|(r, &s)| Some(s / r.as_ref()?.height_scale()))
        .filter(|&f| f == 0.5)
        .unwrap_or(1.0);
    // `height * 1.2 / 78`, a little wide of the legal models' ends.
    let model = |inches: i16| f32::from(inches) * 1.2 / 78.0 * factor;
    let bounds = [
        model(roster_rules::HEIGHT_IN[0] + roster_rules::MODEL_HEIGHT_OFFSET[0]) - 1e-4,
        model(roster_rules::HEIGHT_IN[1] + roster_rules::MODEL_HEIGHT_OFFSET[1]) + 1e-4,
    ];
    for (k, ((record, &number), (&scale, &ratio))) in lineup
        .iter()
        .zip(records)
        .zip(scales.iter().zip(ratios))
        .enumerate()
    {
        let who = format!("world player {k} (record {number})");
        let Some(r) = record else {
            if !(bounds[0]..=bounds[1]).contains(&scale) || !(1.0..=2.0).contains(&ratio) {
                out.push(format!(
                    "{who}: staged scale {scale} and width ratio {ratio} are no legal model's"
                ));
            }
            continue;
        };
        let want = r.height_scale() * factor;
        if scale.to_bits() != want.to_bits() {
            out.push(format!(
                "{who}: staged scale {scale}, the record's {want} (a {} in model)",
                r.model_height_in
            ));
        }
        let want = street_assets::clip::native_body_width_ratio(r.native_body_scale());
        if ratio.to_bits() != want.to_bits() {
            out.push(format!(
                "{who}: staged width ratio {ratio}, the record's {want}"
            ));
        }
    }
    out
}

/// Equipped dribble moves in the world: rows of the right kind and level,
/// and the rows the roster declares where it declares them.
fn check_tricks(world: &World, m: &MatchRoster, records: &[usize]) -> Vec<String> {
    let mut out = Vec::new();
    for (k, (p, &record)) in world.players.iter().zip(records).enumerate() {
        let Some(tricks) = &p.native_tricks else {
            continue;
        };
        let declared = m
            .roster
            .anims
            .iter()
            .find(|(r, _)| usize::from(*r) == record)
            .and_then(|(_, a)| a.rows[2].clone());
        for (slot, t) in tricks.slots.iter().enumerate() {
            if let Some(row) = t.equipped_row
                && roster_rules::move_row(usize::from(row)) != Some((2, t.level))
            {
                out.push(format!(
                    "world player {k}: dribble-move slot {slot} holds row {row}, not a level {} dribble move",
                    t.level
                ));
            }
            if let Some(rows) = &declared {
                let want = rows.get(slot).copied().filter(|&r| r != EMPTY);
                if t.equipped_row != want {
                    out.push(format!(
                        "world player {k}: dribble-move slot {slot} holds {:?}, the roster's {want:?}",
                        t.equipped_row
                    ));
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use street_assets::roster_share::{SharedPlayer, SharedRoster, blank, with_fields};

    fn record(index: usize, rating: i16, height: i16) -> Player {
        let mut f = vec![0i16; 101];
        f[8] = height;
        f[9] = 215;
        f[10] = height;
        f[11] = 215;
        f[16..27].fill(rating);
        f[68..101].fill(-1);
        with_fields(blank(index), f)
    }

    /// A setup as the game makes it: the world from the line-up's records.
    fn setup(lineup: &[Player]) -> MatchSetup {
        let mut s = MatchSetup::standard(Default::default(), 3);
        s.roster = lineup.iter().map(|p| p.index as u16).collect();
        for (w, p) in s.world.players.iter_mut().zip(lineup) {
            let r = |k: usize| f32::from(p.ratings[k]) / 99.0;
            w.ratings = street_sim::Ratings {
                shots: r(0),
                threes: r(1),
                dunks: r(2),
                rebounds: r(3),
                blocks: r(5),
                steals: r(6),
                handles: r(7),
                speed: r(8),
                power: r(4),
                overall: r(9),
                reach: reach_for_height(f32::from(p.height_in)),
            };
        }
        s
    }

    fn packed(lineup: &[Player], players: Vec<SharedPlayer>) -> Vec<u8> {
        MatchRoster {
            roster: SharedRoster {
                players_len: 241,
                teams_len: 49,
                players,
                ..Default::default()
            },
            lineup: lineup.to_vec(),
        }
        .pack()
        .unwrap()
    }

    #[test]
    fn a_legal_roster_and_its_world_pass() {
        let mut lineup: Vec<Player> = (0..6).map(|k| record(k, 60, 78)).collect();
        // An added player, edited legally.
        lineup[5] = record(240, 70, 82);
        let mut s = setup(&lineup);
        s.roster_data = packed(
            &lineup,
            vec![SharedPlayer {
                record: 240,
                source: 7,
                fields: vec![(8, 82), (10, 82), (16, 70)],
                ..Default::default()
            }],
        );
        let m = check_setup(&s, &Known::default()).unwrap().unwrap();
        assert_eq!(m.lineup.len(), 6);
        // The disc's roster: a standard world is legal.
        assert_eq!(
            check_setup(
                &MatchSetup::standard(Default::default(), 1),
                &Known::default()
            ),
            Ok(None)
        );
    }

    #[test]
    fn the_staged_players_scales_and_width_ratios_are_the_records() {
        let mut lineup: Vec<Player> = (0..6).map(|k| record(k, 60, 78)).collect();
        lineup[2] = record(2, 60, 84);
        let mut base = setup(&lineup);
        base.roster_data = packed(&lineup, Vec::new());
        let staged = |scales: Vec<f32>, ratios: Vec<f32>| {
            let mut s = base.clone();
            s.world.staged = Some(std::sync::Arc::new(street_sim::gamebreaker::Config {
                player_scales: scales,
                player_width_ratios: ratios,
                ..Default::default()
            }));
            check_setup(&s, &Known::default())
        };
        let scales: Vec<f32> = lineup.iter().map(Player::height_scale).collect();
        let ratios: Vec<f32> = lineup
            .iter()
            .map(|p| street_assets::clip::native_body_width_ratio(p.native_body_scale()))
            .collect();
        assert!(staged(scales.clone(), ratios.clone()).is_ok());
        // The tiny players' code halves everyone's scale alike.
        let half: Vec<f32> = scales.iter().map(|s| s * 0.5).collect();
        assert!(staged(half, ratios.clone()).is_ok());
        // One player bigger, or alone halved: refused.
        for k in [1.1, 0.5] {
            let mut one = scales.clone();
            one[2] *= k;
            let why = staged(one, ratios.clone()).unwrap_err().reasons();
            assert!(why.iter().any(|w| w.contains("staged scale")), "{why:?}");
        }
        // A wider body, or a list that misses a player.
        let mut wide = ratios.clone();
        wide[4] = 1.25;
        let why = staged(scales.clone(), wide).unwrap_err().reasons();
        assert!(why.iter().any(|w| w.contains("width ratio")), "{why:?}");
        assert!(staged(scales[..5].to_vec(), ratios[..5].to_vec()).is_err());
    }

    #[test]
    fn illegal_rosters_and_worlds_that_differ_are_refused() {
        let lineup: Vec<Player> = (0..6).map(|k| record(k, 60, 78)).collect();
        let base = {
            let mut s = setup(&lineup);
            s.roster_data = packed(&lineup, Vec::new());
            s
        };
        assert!(check_setup(&base, &Known::default()).is_ok());
        // A 96-inch player in the roster.
        let mut s = base.clone();
        s.roster_data = packed(
            &lineup,
            vec![SharedPlayer {
                record: 3,
                source: 3,
                fields: vec![(8, 96)],
                ..Default::default()
            }],
        );
        let why = check_setup(&s, &Known::default()).unwrap_err().reasons();
        assert!(why.iter().any(|w| w.contains("height 96")), "{why:?}");
        // A line-up record with super ratings.
        let mut god = lineup.clone();
        god[0] = record(0, 99, 78);
        let mut s = setup(&god);
        s.roster_data = packed(&god, Vec::new());
        assert!(check_setup(&s, &Known::default()).is_err());
        // A legal roster, but the world plays other numbers.
        let mut s = base.clone();
        s.world.players[4].ratings.dunks = 1.0;
        let why = check_setup(&s, &Known::default()).unwrap_err().reasons();
        assert!(why.iter().any(|w| w.contains("dunks")), "{why:?}");
        let mut s = base.clone();
        s.world.players[1].ratings.reach = reach_for_height(90.0);
        assert!(check_setup(&s, &Known::default()).is_err());
        // Not even a few points for the home side.
        let mut s = base.clone();
        s.world.players[0].ratings.dunks = 70.0 / 99.0;
        assert!(check_setup(&s, &Known::default()).is_err());
        // A cheat code sets everyone's speed alike: allowed.
        let mut s = base.clone();
        for p in &mut s.world.players {
            p.ratings.speed = 1.0;
        }
        assert!(check_setup(&s, &Known::default()).is_ok());
        // Damaged data.
        let mut s = base.clone();
        s.roster_data = b"RSTZ\x01nope".to_vec();
        assert!(check_setup(&s, &Known::default()).is_err());
        // Without roster data: no added players, and legal world players.
        let mut s = setup(&lineup);
        s.roster[0] = 250;
        assert!(check_setup(&s, &Known::default()).is_err());
        let mut s = setup(&lineup);
        for p in &mut s.world.players {
            p.ratings.dunks = 1.0;
            p.ratings.shots = 1.0;
            p.ratings.blocks = 1.0;
            p.ratings.steals = 1.0;
            p.ratings.handles = 1.0;
            p.ratings.rebounds = 1.0;
        }
        assert!(check_setup(&s, &Known::default()).is_err());
    }

    #[test]
    fn a_mod_base_needs_the_mod_or_trust() {
        let mut lineup: Vec<Player> = (0..6).map(|k| record(k, 60, 78)).collect();
        // The mod's own star (a generic record), over the cap.
        lineup[0] = record(231, 95, 78);
        let mut s = setup(&lineup);
        let id = [9; 32];
        s.roster_data = MatchRoster {
            roster: SharedRoster {
                base: Base::Mod(id),
                players_len: 240,
                teams_len: 49,
                players: vec![SharedPlayer {
                    record: 231,
                    source: 231,
                    fields: (16..27).map(|f| (f, 95)).collect(),
                    ..Default::default()
                }],
                ..Default::default()
            },
            lineup: lineup.clone(),
        }
        .pack()
        .unwrap();
        // A server without the mod refuses it, saying which.
        let none = Known::default();
        let err = check_setup(&s, &none).unwrap_err();
        assert_eq!(err, RosterError::UnknownMod(id));
        assert!(err.reasons()[0].contains("doesn't have the 0909"));
        // A server holding it checks against its records.
        let mut base: Vec<Player> = (0..240).map(|k| record(k, 60, 78)).collect();
        base[231] = record(231, 95, 78);
        let held = Known {
            mods: vec![ModTables {
                id,
                players: base.clone(),
                teams: Vec::new(),
            }],
            ..Default::default()
        };
        assert!(check_setup(&s, &held).is_ok());
        // ... and refuses a record edited past its base.
        base[231] = record(231, 90, 78);
        let lower = Known {
            mods: vec![ModTables {
                id,
                players: base,
                teams: Vec::new(),
            }],
            ..Default::default()
        };
        assert!(matches!(
            check_setup(&s, &lower),
            Err(RosterError::Illegal(_))
        ));
        // One that trusts mods takes it.
        let trusting = Known {
            trust_mods: true,
            ..Default::default()
        };
        assert!(check_setup(&s, &trusting).is_ok());
    }

    #[test]
    fn with_the_disc_the_lineup_must_be_the_discs() {
        let tables = DiscTables {
            players: (0..240).map(|k| record(k, 50, 78)).collect(),
            teams: (0..49)
                .map(|t| Team {
                    index: t,
                    name: format!("T{t}"),
                    city: String::new(),
                    nickname: String::new(),
                    roster: vec![],
                    logo: 0,
                })
                .collect(),
        };
        let lineup: Vec<Player> = (0..6).map(|k| tables.players[k].clone()).collect();
        let disc = Known {
            disc: Some(tables),
            ..Default::default()
        };
        let mut s = setup(&lineup);
        s.roster_data = packed(&lineup, Vec::new());
        assert!(check_setup(&s, &disc).is_ok());
        // Better than the disc's record: legal values, but not the disc's.
        let mut lie = lineup.clone();
        lie[2] = record(2, 60, 78);
        let mut s = setup(&lie);
        s.roster_data = packed(&lie, Vec::new());
        assert!(check_setup(&s, &Known::default()).is_ok());
        assert!(check_setup(&s, &disc).is_err());
        // Without roster data the world must hold the disc's values.
        let mut s = setup(&lie);
        s.roster_data.clear();
        assert!(check_setup(&s, &disc).is_err());
        let s = setup(&lineup);
        assert!(check_setup(&s, &disc).is_ok());
    }
}
