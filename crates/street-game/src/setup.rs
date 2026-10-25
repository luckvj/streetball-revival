//! The disc, read once at start, and getting a match ready from it.

use std::{collections::BTreeMap, fs::File, io::BufReader, sync::Arc};

use bevy::prelude::*;
use street_assets::{court, courtfx, db, eagl, iso::Iso, player, ssh};
use street_render::{
    animation::{Clip, Clips},
    player::{Choice, PlayerAssets},
};
use street_sim::{Hoop, Input, PassStyle, Player, Ratings, Rules};

use crate::{
    controls::{self, Control},
    game,
};

/// Centre of the rim in a hoop model's own space: the middle of the `orim`
/// and `rim1` meshes (x 3.02..5.12, y 2.13..2.63 in every court's hoop).
const RIM_LOCAL: Vec3 = Vec3::new(4.07, 2.45, 0.0);
pub(crate) const RIM_RADIUS: f32 = 0.75;
/// Players per side.
pub const TEAM_SIZE: usize = 3;

/// Where the disc image is, for work that reads it off the main thread.
#[derive(Resource, Clone)]
pub struct DiscPath(pub std::path::PathBuf);

/// Match-scoped selectors, including a remote host's presentation settings.
#[derive(Resource, Clone)]
pub struct MatchCheats(pub crate::cheats::Settings);

/// What the Disc holds that the menus and every match need.
#[derive(Resource)]
pub struct Disc {
    pub iso: Iso<BufReader<File>>,
    pub players: Vec<db::Player>,
    pub teams: Vec<db::Team>,
    pub courts: Vec<db::Court>,
    pub cards: Vec<db::Card>,
    pub ball: (eagl::Model, BTreeMap<String, ssh::Texture>),
    /// How many teams the database holds; created teams follow them.
    pub base_teams: usize,
    /// The Rewards screens' strings (`poster.txt` of the front end's
    /// locale), for [`crate::rewards`]; empty if missing.
    pub poster: Vec<String>,

    /// The shared strings (`common.txt`), the cheats' names among them;
    /// empty if missing.
    pub common: Vec<String>,
    /// The named moves (`XDBMOVE`), for Create A Baller's moves page.
    pub moves: Vec<db::Move>,
    /// The High Scores strings (`score.txt`); empty if missing.
    pub score: Vec<String>,
    /// Trax's strings (`bigradio.txt`); empty if missing.
    pub radio: Vec<String>,
    /// The nicknames a baller can earn (`XDBNICK`), each with its skill.
    pub nicknames: Vec<db::Nickname>,
    pub nickname_intro: Vec<String>,
    pub nickname_flavor: Vec<String>,
    /// Be A Legend's event intros (`pickupintro.txt`, `tourneyintro.txt`,
    /// `streetchallengeintro.txt`); empty if missing.
    pub intros: [Vec<String>; 3],
}

impl Disc {
    pub fn load(mut iso: Iso<BufReader<File>>) -> Result<Self, Box<dyn std::error::Error>> {
        let teams = db::teams(&mut iso)?;
        let locale =
            street_assets::locale::read(&mut iso, "/LOCALE/XLOC_FE.EUS").unwrap_or_default();
        let text = |name: &str| {
            locale
                .iter()
                .find(|f| f.name == name)
                .map(|f| f.texts.clone())
                .unwrap_or_default()
        };
        let (poster, score) = (text("poster.txt"), text("score.txt"));
        let radio = text("bigradio.txt");
        let intros = [
            text("pickupintro.txt"),
            text("tourneyintro.txt"),
            text("streetchallengeintro.txt"),
        ];
        let moves = db::moves(&mut iso).unwrap_or_default();
        let gm = street_assets::locale::read(&mut iso, "/LOCALE/XLOC_GM.EUS").unwrap_or_default();
        let gm_text = |name: &str| {
            gm.iter()
                .find(|f| f.name == name)
                .map(|f| f.texts.clone())
                .unwrap_or_default()
        };
        Ok(Self {
            poster,
            common: text("common.txt"),
            score,
            radio,
            nicknames: db::nicknames(&mut iso).unwrap_or_default(),
            nickname_intro: gm_text("givenickname.txt"),
            nickname_flavor: gm_text("nicknames.txt"),
            intros,
            moves,
            players: db::players(&mut iso)?,
            base_teams: teams.len(),
            teams,
            courts: db::courts(&mut iso)?,
            cards: db::cards(&mut iso)?,
            ball: court::ball(&mut iso, 'a')?,
            iso,
        })
    }

    /// Teams with at least three players, in database order: the league,
    /// then the legends and street teams.
    pub fn playable_teams(&self) -> Vec<usize> {
        self.teams
            .iter()
            .filter(|t| t.roster.len() >= TEAM_SIZE)
            .map(|t| t.index)
            .collect()
    }

    /// The team whose name, nickname, city or full name is `name`.
    pub fn find_team(&self, name: &str) -> Result<usize, String> {
        let wanted = name.to_ascii_lowercase();
        self.teams
            .iter()
            .filter(|t| t.roster.len() >= TEAM_SIZE)
            .find(|t| {
                [&t.name, &t.nickname, &t.city, &t.full_name()]
                    .iter()
                    .any(|n| n.to_ascii_lowercase() == wanted)
            })
            .map(|t| t.index)
            .ok_or_else(|| format!("no team called {name:?} with three players"))
    }

    /// A court's display name for its archive id.
    pub fn court_name(&self, id: &str) -> String {
        self.courts
            .iter()
            .find(|c| c.id.eq_ignore_ascii_case(id))
            .map_or_else(|| id.to_owned(), |c| c.name.clone())
    }
}

/// `--full-meters` / `--level1`: every match starts with the home team's
/// Level 2 (or Level 1) gamebreaker ready (a testing aid).
#[derive(Resource)]
pub struct FullMeters {
    /// A gamebreaker pocketed under the full meter: Level 2.
    pub level2: bool,
}

/// `--level2-at x,z`: in live play the away team's meter fills with one
/// pocketed (a Level 2, as the School's lesson fills it, the meter's
/// listeners hearing it) and, 40 updates on, once the away team holds the
/// ball, its holder is put at `(x, z)` and fires the gamebreaker on his own
/// pad: two turbos, then shoot with them held (a testing aid, the way the
/// original's School driver plays the lesson, so a staged plan can be
/// compared with the original's at its region).
#[derive(Resource)]
pub struct Level2At {
    at: [f32; 2],
    stage: u8,
    filled: u64,
}

impl Level2At {
    pub fn new(at: [f32; 2]) -> Self {
        Self {
            at,
            stage: 0,
            filled: 0,
        }
    }

    /// After the step, before its events are read: the fill, at the first
    /// update of live play.
    pub fn fill(&mut self, sim: &mut game::Sim) {
        if self.stage == 0 && matches!(sim.world.phase, street_sim::Phase::Live) {
            sim.world.teams[1].pocketed = true;
            sim.world
                .set_meter(1, street_sim::street::METER_FULL as f32);
            self.filled = sim.world.tick;
            self.stage = 1;
        }
    }

    /// Before the step: this update's pad for the away holder, if due.
    pub fn drive(&mut self, sim: &mut game::Sim) {
        use street_sim::button;
        if self.stage >= 3 || !matches!(sim.world.phase, street_sim::Phase::Live) {
            return;
        }
        if self.stage == 0 {
            return;
        }
        let Some(holder) = sim
            .world
            .handler()
            .filter(|&h| sim.world.players[h].team == 1)
        else {
            return;
        };
        if self.stage == 1 {
            if sim.world.tick < self.filled + 40 {
                return;
            }
            sim.world.players[holder].position = self.at;
        }
        let shoot = if self.stage == 2 { button::SHOOT } else { 0 };
        sim.inputs[holder] = street_sim::Input::default().with(button::L1 | button::R1 | shoot);
        self.stage += 1;
    }
}

/// The animations, by name, decoded once at start.
#[derive(Resource)]
pub struct GameClips {
    pub clips: Clips,
    pub scripts: street_assets::scripts::MoveScripts,
    /// Source-derived ordinary metadata; actor execution remains separate.
    pub gait: street_assets::gait::GaitData,
    /// Bounded source program data; no actor VM executes during asset loading.
    pub ordinary: street_assets::ordinary::OrdinaryPrograms,
    pub timeline: street_assets::timeline::ScriptBank,
    /// The move scripts the display plays on the VM's clock
    /// (`crate::channel`).
    pub programs: crate::channel::Programs,
    /// What the simulation reads from the disc (the pose bank, the gait
    /// records, the staged plans) and its digest: `street-net`'s `disc`,
    /// built as every machine in an online match builds it.
    pub sim: Arc<street_net::Disc>,
}

impl GameClips {
    pub fn get(&self, name: &str) -> Arc<Clip> {
        self.clips.get(name).expect("loaded at start")
    }
}

/// The match the menus have set up.
#[derive(Resource, Clone, Debug)]
pub struct MatchSetup {
    /// Home and away teams (database team indices).
    pub teams: [usize; 2],
    /// The three players of each side (database player records).
    pub lineups: [[usize; TEAM_SIZE]; 2],
    /// Court archive id, e.g. RUCKER.
    pub court: String,
    pub rules: Rules,
    pub seed: u32,
    /// The computer plays the home team too.
    pub demo: bool,
    /// The command line's `--demo`: the computer plays every match, the
    /// story's too (for scripted runs).
    pub watch: bool,
    /// Who plays which side (0 home, 1 away), in the order they joined: 1P
    /// first. Empty: one person on the home team on every controller.
    pub seats: Vec<(controls::Device, usize)>,
    /// The name each seat plays under (`None` a guest), by seat; see
    /// [`crate::highscores`].
    pub users: Vec<Option<String>>,
    /// Played online ([`crate::online`]): people on both sides, so no help
    /// for the home team.
    pub online: bool,
}

impl MatchSetup {
    /// Home against away with the first three of each roster, at Rucker.
    pub fn new(disc: &Disc, home: usize, away: usize) -> Self {
        let lineup = |t: usize| {
            let roster = &disc.teams[t].roster;
            std::array::from_fn(|k| roster[k])
        };
        Self {
            teams: [home, away],
            lineups: [lineup(home), lineup(away)],
            court: "RUCKER".into(),
            rules: Rules::default(),
            seed: 2002,
            demo: false,
            watch: false,
            seats: Vec::new(),
            users: Vec::new(),
            online: false,
        }
    }
}

/// The match's team names: each side's nickname (`.0`, what the
/// rewrite's own lines say) and city (`.1`). The original's boards draw
/// the two on elements of their own, the city over the name (The Rundown,
/// `FUN_002c7b48`: `FUN_001ed7d0` into element 2, `FUN_001ed890` into 10).
/// Every machine in an online match has the creator's ([`team_names`]).
#[derive(Resource)]
pub struct TeamNames(pub [String; 2], pub [String; 2]);

/// Each side's nickname and city: online, the creator's records' as the
/// match's looks carry them (a created team is another disc's base team on
/// a joiner's disc; the creator, whose match is already loaded, made the
/// looks from these same records); else this disc's records.
pub fn team_names(
    remote: Option<&crate::net_looks::NetLooks>,
    teams: &[db::Team],
    sides: [usize; 2],
) -> TeamNames {
    let pick = |side: usize| -> (String, String) {
        remote
            .map(|r| r.teams[side].clone())
            .filter(|(_, name)| !name.is_empty())
            .unwrap_or_else(|| {
                let team = &teams[sides[side]];
                (team.city.clone(), team.nickname.clone())
            })
    };
    let [(city0, name0), (city1, name1)] = [pick(0), pick(1)];
    TeamNames([name0, name1], [city0, city1])
}

#[derive(Resource)]
pub struct PlayerNames(pub Vec<String>);

/// Skeleton index of the right hand, which carries the ball.
#[derive(Resource)]
pub struct HandJoint(pub usize);

/// Skeleton index of the jaw, where breath shows in the cold.
#[derive(Resource)]
pub struct MouthJoint(pub usize);

/// The match's models, to spawn.
#[derive(Resource)]
pub struct Scene {
    pub court: court::Court,
    pub players: Vec<PlayerAssets>,
    /// The referee of the opening toss (`crate::referee`).
    pub referee: Option<PlayerAssets>,
    /// The court's effects script (`/COURTFX`): sun, shadows, litter, traffic.
    pub fx: courtfx::CourtFx,
    /// The court's crowd, if it has one.
    pub crowd: Option<street_assets::crowd::Crowd>,
    /// Its litter, steam and breath ([`crate::ambience`]).
    pub ambience: crate::ambience::AmbientAssets,
}

/// The simulation's view of a database player, with the crew's Gym bump
/// added to each skill (the save's `upgrades`, one point per buy).
pub fn ratings(p: &db::Player, bump: &[i16; 9]) -> Ratings {
    let raw = |skill: usize| p.ratings[skill].saturating_add(bump[skill]);
    let r = |skill: usize| (f32::from(raw(skill)) / 99.0).clamp(0.0, 1.0);
    Ratings {
        shots: r(0),
        threes: r(1),
        dunks: r(2),
        rebounds: r(3),
        blocks: r(5),
        steals: r(6),
        handles: r(7),
        speed: r(8),
        power: r(4),
        overall: (f32::from(overall(p, raw(0), raw(2))) / 99.0).clamp(0.0, 1.0),
        reach: street_sim::player::reach_for_height(f32::from(p.height_in)),
    }
}

/// Rating 9 as `FUN_001d7d00` reads it: the record's field 25, or for a
/// created player (team 0x25) the table `0x45fdc0` at the sum of his shots
/// and dunks bars (`FUN_001d7f48`, 0..6 each; the sum 12, both bars full,
/// reads the table's 0).
fn overall(p: &db::Player, shots: i16, dunks: i16) -> i16 {
    const TABLE: [i16; 14] = [0, 10, 20, 30, 40, 50, 60, 70, 80, 90, 95, 99, 0, 0];
    if p.team == db::CREATED_PLAYER_TEAM {
        use crate::progression_trace::skill_bar;
        let sum = skill_bar(db::Skill::Shots, shots) + skill_bar(db::Skill::Dunks, dunks);
        TABLE[usize::from(sum)]
    } else {
        p.ratings[9]
    }
}

/// The two rims (the +x basket first, the one team 0 attacks): the
/// original's ring centres ([`Hoop::native`]), checked against the hoops
/// the court places. The
/// meshes' middle sits 0.2 ft toward the board (their bounds take in the
/// collar) and 0.16 ft lower than the ring's plane.
pub(crate) fn hoops(court: &court::Court) -> Result<[Hoop; 2], String> {
    let mut rims: Vec<Vec3> = court
        .placements
        .values()
        .map(|p| Vec3::from_array(p.translation) + Quat::from_array(p.rotation) * RIM_LOCAL)
        .collect();
    rims.sort_by(|a, b| b.x.total_cmp(&a.x));
    let native = Hoop::native();
    match rims[..] {
        [a, b]
            if [a, b]
                .iter()
                .zip(&native)
                .all(|(placed, hoop)| (*placed - Vec3::from_array(hoop.rim)).length() < 0.5) =>
        {
            Ok(native)
        }
        [_, _] => Err(format!(
            "court {} places its hoops away from the original's rims",
            court.id
        )),
        _ => Err(format!("court {} does not place two hoops", court.id)),
    }
}

/// Read everything the set-up match needs from the disc and put its
/// resources in place: the scene to spawn, the simulation, the names.
pub fn load_match(world: &mut World) -> Result<(), Box<dyn std::error::Error>> {
    let setup = world.resource::<MatchSetup>().clone();
    let table = world.resource::<Disc>();
    if setup.teams.iter().any(|&team| team >= table.teams.len())
        || setup
            .lineups
            .iter()
            .flatten()
            .any(|&player| player >= table.players.len())
    {
        return Err("The match references a team or player outside this disc's database.".into());
    }
    let remote = if setup.online {
        world.get_resource::<crate::net_looks::NetLooks>().cloned()
    } else {
        None
    };
    let cheats = remote
        .as_ref()
        .map(|r| r.cheats.clone())
        .unwrap_or_else(|| {
            world
                .get_resource::<crate::cheats::Settings>()
                .cloned()
                .unwrap_or_default()
        });
    let ballers = remote
        .as_ref()
        .map(|r| r.ballers.clone())
        .unwrap_or_else(|| {
            world
                .get_resource::<crate::story::Save>()
                .map(|s| s.story.ballers.clone())
                .unwrap_or_default()
        });
    let moves = crate::clip::PlayerMoves(
        setup
            .lineups
            .iter()
            .flatten()
            .map(|&r| {
                (
                    r,
                    ballers
                        .iter()
                        .find(|b| b.record() == r)
                        .map(|b| b.moves.clone())
                        .unwrap_or_default(),
                )
            })
            .collect(),
    );
    let crew = world
        .get_resource::<crate::story::Save>()
        .map(|save| save.story.crew.clone())
        .unwrap_or_default();
    let gym = world
        .get_resource::<crate::story::Save>()
        .map(|save| save.story.upgrades)
        .unwrap_or([0; 9]);
    let sixth_men = world
        .get_resource::<crate::story::Save>()
        .map(|save| save.story.sixth_men.clone())
        .unwrap_or_default();
    let run = world
        .get_resource::<crate::story::Running>()
        .and_then(|r| r.0);
    let progress = world
        .get_resource::<crate::story::Save>()
        .map(|save| save.story.clone());
    if let Some(progress) = &progress {
        crate::story::apply_progression_fields(progress, &mut world.resource_mut::<Disc>());
    }
    let meter_weight = progress.as_ref().and_then(|progress| {
        crate::story::meter_weight(run, progress, world.resource::<Disc>(), &setup)
    });
    let ball_suffix = progress
        .as_ref()
        .map(|progress| crate::story::match_ball_suffix(&cheats, run, progress))
        .unwrap_or_else(|| cheats.ball_model_suffix());
    let native_tricks = world
        .get_resource::<GameClips>()
        .map(|clips| {
            let disc = world.resource::<Disc>();
            (0..moves.0.len())
                .map(|player| {
                    crate::clip::resolved_trick_slots(clips, &moves, disc, player)
                        .ok_or("The player's native trick slots do not match this disc.")
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?;
    // Which dunk slots hold an advanced dunk (the dunk ids' flag 0x800).
    let native_dunks: Option<Vec<Option<u16>>> = world.get_resource::<GameClips>().map(|clips| {
        let disc = world.resource::<Disc>();
        (0..moves.0.len())
            .map(|player| crate::clip::advanced_dunk_slots(clips, &moves, disc, player))
            .collect()
    });
    let mut disc = world.resource_mut::<Disc>();
    disc.ball = court::ball(&mut disc.iso, ball_suffix)?;
    let court = court::load(&mut disc.iso, &setup.court)?;
    let hoops = hoops(&court)?;
    // A court without an effects script gets the game's defaults.
    let fx = courtfx::CourtFx::read(&mut disc.iso, &setup.court).unwrap_or_default();
    let ambience = crate::ambience::AmbientAssets::load(&mut disc.iso, &fx);
    // Broad Street has no crowd; any other failure leaves the stands empty.
    let crowd = match street_assets::crowd::load(&mut disc.iso, &setup.court) {
        Ok(c) => Some(c),
        Err(street_assets::Error::NotFound(_)) => None,
        Err(e) => {
            bevy::log::warn!("crowd: {e}");
            None
        }
    };
    let referee = match PlayerAssets::referee(&mut disc.iso) {
        Ok(referee) => Some(referee),
        Err(e) => {
            warn!("referee: {e}");
            None
        }
    };
    let mut sim_players = Vec::new();
    let mut gait_profiles = Vec::new();
    let mut assets = Vec::new();
    let mut names = Vec::new();
    for (team, lineup) in setup.lineups.iter().enumerate() {
        // The home team wears its home uniform (white for most, gold for the Lakers).
        let uniform = if team == 0 {
            player::Side::Home
        } else {
            player::Side::Road
        };
        for (k, &record) in lineup.iter().enumerate() {
            // A remote career star past this disc's records starts from
            // the create-a-player template, as `ballers::apply_all` does.
            let template = usize::from(street_sim::story::BALLER_RECORDS);
            let mut p = disc
                .players
                .get(record)
                .unwrap_or(&disc.players[template])
                .clone();
            if let Some(b) = remote
                .as_ref()
                .and_then(|r| r.ballers.iter().find(|b| b.record() == record))
            {
                crate::ballers::apply(&mut p, b);
            }
            // The record as the Disc holds it: created ballers carry their edits.
            let mut look = PlayerAssets::load(
                &mut disc.iso,
                &Choice::Player {
                    record: Box::new(p.clone()),
                    side: uniform,
                    // A created player playing for a league team, or in the
                    // team uniform (code 80), wears the uniform of the team
                    // he joined (0x31590c..0x315944: `FUN_001d84d0`).
                    team: (setup.teams[team] < 29 || p.clothing & 0x3fff == 80)
                        .then(|| sixth_men.iter().find(|&&(_, r)| usize::from(r) == record))
                        .flatten()
                        .map(|&(t, _)| usize::from(t)),
                },
            )?;
            look.scale *= cheats.body_scale();
            look.head_scale = cheats.head_scale();
            // Lined up in their own half, facing the basket they attack (the
            // home team's is at +x, on the left of the screen).
            let side = street_sim::restart::side(team);
            let x = -12.0 * side;
            let heading = 1.57 * side;
            // The crew carries its Gym development into every match.
            let bump = if team == 0 && crew.contains(&(record as u16)) {
                gym
            } else {
                [0; 9]
            };
            let mut sim_ratings = ratings(&p, &bump);
            cheats.apply_ratings(&mut sim_ratings);
            // A roster file may borrow another player's walks and runs.
            let (gait_identity, gait_weight, gait_female) =
                street_assets::roster_edits::gait_source(record).unwrap_or((
                    record as i32,
                    p.weight_lb,
                    p.female,
                ));
            gait_profiles.push(street_sim::gait_select::GaitProfile {
                category: street_sim::gait_select::GaitProfile::category_for(
                    i32::from(gait_weight),
                    gait_female,
                ),
                identity: gait_identity,
                handles: (sim_ratings.handles * 99.0).round() as i32,
            });
            // The contact manager shares a push by the record's weight
            // (attribute 9, `FUN_001da4e0(record, 9)`).
            let mut player = Player::new(team, [x, (k as f32 - 1.0) * 9.0], heading)
                .with_ratings(sim_ratings)
                .with_weight(p.weight_lb.max(1) as u16);
            // The model's scales for the bone rules (`FUN_003153d0`, the
            // tiny-player and head-size codes as the model takes them).
            player.body = p
                .native_body_scale()
                .map(|[height, width, _]| street_sim::pose::Body {
                    height: height * cheats.body_scale(),
                    width: width * cheats.body_scale(),
                    head: p.native_head_scale().map_or(1.0, |h| h[0]) * cheats.head_scale(),
                });
            // Field 13 (actor+0xc0 2): a left-hander's blends take their
            // `f2` alternates.
            player.left_handed = p.left_handed;
            sim_players.push(player);
            names.push(p.name());
            assets.push(look);
        }
    }
    let hand = assets[0]
        .skeleton
        .bones
        .iter()
        .position(|b| b.name == "RHand")
        .ok_or("player skeleton has no RHand")?;
    let mouth = assets[0]
        .skeleton
        .bones
        .iter()
        .position(|b| b.name == "jaw");
    let teams = team_names(remote.as_ref(), &disc.teams, setup.teams);
    let mut rules = setup.rules;
    cheats.apply_rules(&mut rules);
    // Built on the session's generator as it stands, as the original
    // builds its game on the shared one (`session.rs`); a run without a
    // session (a test) starts one from the setup's seed.
    if !world.contains_resource::<crate::session::Session>() {
        world.insert_resource(crate::session::Session::pinned(setup.seed));
    }
    let generator = world.resource::<crate::session::Session>().generator();
    let mut sim = street_sim::World::from_generator(hoops, sim_players, rules, generator);
    if let Some(tricks) = native_tricks {
        for (state, tricks) in sim.players.iter_mut().zip(tricks) {
            state.native_tricks = Some(tricks);
        }
    }
    if let Some(dunks) = native_dunks {
        for (state, mask) in sim.players.iter_mut().zip(dunks) {
            state.native_dunks = mask;
        }
    }
    sim.ball.radius = cheats.ball_radius();
    sim.ball.position[1] = sim.ball.radius;
    // The court's ceiling, on the two courts that have one.
    if let Some(court) = world
        .resource::<Disc>()
        .courts
        .iter()
        .find(|c| c.id.eq_ignore_ascii_case(&setup.court))
    {
        sim.ball.ceiling = street_sim::ball::ceiling_for_court(court.index);
    }
    // A match opens with the referee's toss at centre court (game phase 1,
    // `FUN_00141fd8`), not an inbound; the ball waits in his hand.
    if sim.players.len() == 6 {
        sim.start_opening_toss();
    }
    // The disc's data under the match's own values, installed as every
    // machine in an online match installs it (`street-net`'s `disc`): the
    // pose bank for the rules that read bones, the original's 108 gait
    // records for walks, jogs and runs under each player's profile, and
    // the staged Level 2 gamebreakers (`SCRIPTS.BIN`) under each player's
    // model scale and width. A match never plays without them.
    let staged_scales: Vec<f32> = assets.iter().map(|player| player.scale).collect();
    let staged_widths: Vec<f32> = assets
        .iter()
        .map(|player| street_render::animation::native_body_width_ratio(player.native_body_scale))
        .collect();
    match world.get_resource::<GameClips>() {
        Some(clips) => {
            sim.set_match_values(gait_profiles, staged_scales, staged_widths);
            clips.sim.install(&mut sim)?;
        }
        // Only a test build plays without them (`street_sim::pose::Bank`).
        None if sim.poses.is_missing() => {
            return Err("The disc's animations are not loaded.".into());
        }
        None => {}
    }
    let controlled = if setup.online {
        [true, true]
    } else if setup.demo {
        [false, false]
    } else if setup.seats.is_empty() {
        [true, false]
    } else {
        std::array::from_fn(|side| setup.seats.iter().any(|&(_, team)| team == side))
    };
    sim.configure_meter_fill(controlled, meter_weight);
    // Each side's database team, which the computer's pocket and cancel
    // checks read for Team Whitewater (`street_sim::World::side_teams`).
    sim.side_teams = setup
        .teams
        .map(|t| u16::try_from(t).unwrap_or(street_sim::NO_TEAM));
    if let Some(level2) = world.get_resource::<FullMeters>().map(|f| f.level2) {
        // The match start's fill (`FUN_001457f8`): 100 and over pockets one,
        // and the rest × 500 goes through the meter's set (`FUN_001448e0`).
        sim.teams[0].pocketed = level2;
        sim.set_meter(0, street_sim::street::METER_FULL as f32);
        // Before the game starts no event reaches the stats (FUN_00186fa8).
        sim.events.clear();
        sim.stats = street_sim::stats::Stats::new(sim.players.len());
    }
    let control = Control::new(&setup.seats, setup.demo, &sim);
    let n = sim.players.len();
    world.insert_resource(game::Sim {
        world: sim,
        inputs: vec![Input::default(); n],
        call: String::new(),
        pass_style: vec![PassStyle::Normal; n],
    });
    world.insert_resource(game::Stats::new(n));
    world.insert_resource(MatchCheats(cheats));
    world.insert_resource(teams);
    world.insert_resource(PlayerNames(names));
    world.insert_resource(moves);
    world.insert_resource(HandJoint(hand));
    if let Some(m) = mouth {
        world.insert_resource(MouthJoint(m));
    } else {
        world.remove_resource::<MouthJoint>();
    }
    world.insert_resource(control);
    world.insert_resource(street_render::shadow::ShadowStyle::from_court(&fx));
    world.insert_resource(Scene {
        court,
        players: assets,
        referee,
        fx,
        crowd,
        ambience,
    });
    Ok(())
}

/// Decode every clip a match uses.
pub fn load_clips(iso: &mut Iso<BufReader<File>>) -> Result<GameClips, Box<dyn std::error::Error>> {
    let scripts = street_assets::scripts::MoveScripts::read(iso)?;
    let gait = street_assets::gait::GaitData::read(iso)?;
    let timeline = street_assets::timeline::ScriptBank::read(iso)?;
    let clips = Clips::load_all(iso)?;
    // What the simulation reads from the disc, built as an online match's
    // referee and clients build it from theirs.
    let sim = street_net::Disc::build(street_net::disc::Parts {
        poses: Arc::new(street_net::disc::poses(iso)?),
        skeleton: &player::skeleton(iso)?,
        clips: &clips,
        timeline: &timeline,
        scripts: &scripts,
        gait: &gait,
        players: &db::Table::read(iso, db::PLAYERS)?,
        teams: &db::Table::read(iso, db::TEAMS)?,
    })?;
    let loaded = GameClips {
        programs: crate::channel::Programs::compile(&iso.read("/SLUS_206.51")?)?,
        scripts,
        gait,
        ordinary: street_assets::ordinary::OrdinaryPrograms::read(iso)?,
        timeline,
        clips,
        sim: Arc::new(sim),
    };
    for name in crate::clip::all() {
        if loaded.clips.get(name).is_none() {
            return Err(format!("required gameplay clip {name:?} is missing").into());
        }
    }
    Ok(loaded)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The disc's gait tables (`street_net::disc::gait`) with `profiles`.
    fn gait_config(
        clips: &GameClips,
        profiles: Vec<street_sim::gait_select::GaitProfile>,
    ) -> street_sim::gait_select::GaitConfig {
        street_sim::gait_select::GaitConfig {
            profiles,
            ..street_net::disc::gait::tables(
                &clips.clips,
                &clips.timeline,
                &clips.scripts,
                &clips.gait,
            )
        }
    }

    #[test]
    fn power_and_the_overall_come_from_the_record() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let players = db::original_players(&mut Iso::open(path).unwrap()).unwrap();
        // Tim Duncan (record 120): power 80 (field 20), overall 90 (25).
        let duncan = &players[120];
        let r = ratings(duncan, &[0; 9]);
        assert_eq!((r.native(4), r.native(9)), (80, 90));
        // The Gym's power bump counts; the overall has no bump.
        let mut bump = [0; 9];
        bump[4] = 5;
        assert_eq!(ratings(duncan, &bump).native(4), 85);
        // A created player's overall (`FUN_001d7d00`): `0x45fdc0` at his
        // shots bar plus his dunks bar (50 is bar 2, 80 bar 4), and the
        // table's 0 at two full bars.
        let mut created = duncan.clone();
        created.team = db::CREATED_PLAYER_TEAM;
        assert_eq!(ratings(&created, &[0; 9]).native(9), 60);
        created.ratings[0] = 99;
        created.ratings[2] = 99;
        assert_eq!(ratings(&created, &[0; 9]).native(9), 0);
    }

    #[test]
    fn the_live_hand_follows_the_held_clips_and_picks_the_gait_row() {
        use street_sim::gait_select::{GaitProfile, HAND_LEFT, HAND_RIGHT};
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let clips = load_clips(&mut Iso::open(path).unwrap()).unwrap();
        let profile = GaitProfile {
            category: 1,
            identity: 20,
            handles: 50,
        };
        let config = gait_config(&clips, vec![profile]);
        // Record 104's rows: 88 RUN_CYCLE_WBALL_05_3_R dribbles right (its
        // frames 2..14 carry state bit 8), its mirrored twin 89 left.
        let hand = |row: u16| config.row_hand(row, 5).unwrap();
        assert_eq!(hand(88), HAND_RIGHT);
        assert_eq!(hand(89), HAND_LEFT);
        // The standing dribble passes the ball hand to hand.
        assert!(config.stand_hands.contains(&HAND_RIGHT));
        assert!(config.stand_hands.contains(&HAND_LEFT));
        assert_eq!(config.trick_hands.len(), clips.scripts.tricks.len());
        // Through the world: a held runner starts on the left (hand 2), so
        // a two-row record plays its alternate; the clip it plays keeps it.
        let mut w = street_sim::World::new(
            Hoop::native(),
            vec![Player::new(0, [0.0, 0.0], 0.0)],
            street_sim::Rules::default(),
            3,
        );
        w.phase = street_sim::Phase::Live;
        w.gait = Some(Arc::new(config.clone()));
        w.ball.hold(0);
        for _ in 0..40 {
            w.step(&[street_sim::Input::moving([1.0, 0.0])]);
        }
        let c = &w.players[0].gait_channel;
        let record = config.records[usize::from(c.record.unwrap())];
        assert_eq!(c.record_row(), record.rows[usize::from(c.hand == 2)]);
        let flags = config.row_hand(c.record_row(), c.phase as usize).unwrap();
        if flags == HAND_LEFT {
            assert_eq!(c.hand, 2);
        } else if flags == HAND_RIGHT {
            assert_eq!(c.hand, 1);
        }
    }

    #[test]
    fn a_crossover_trick_moves_the_live_hand_frame_by_frame() {
        use street_sim::gait_select::{HAND_LEFT, HAND_RIGHT};
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let clips = load_clips(&mut Iso::open(path).unwrap()).unwrap();
        let config = gait_config(&clips, vec![Default::default()]);
        // A trick whose clip starts in the right hand and ends in the left.
        let (script, _) = config
            .trick_hands
            .iter()
            .find(|(_, [primary, _])| {
                primary.iter().find(|&&h| h == HAND_RIGHT || h == HAND_LEFT) == Some(&HAND_RIGHT)
                    && primary
                        .iter()
                        .rev()
                        .find(|&&h| h == HAND_RIGHT || h == HAND_LEFT)
                        == Some(&HAND_LEFT)
            })
            .expect("a right-to-left trick");
        let identity = street_sim::TrickIdentity {
            slot: 0,
            script: *script,
            level: 4,
            equipped_row: None,
        };
        let mut w = street_sim::World::new(
            Hoop::native(),
            vec![Player::new(0, [0.0, 0.0], 0.0)],
            street_sim::Rules::default(),
            3,
        );
        w.phase = street_sim::Phase::Live;
        w.gait = Some(Arc::new(config.clone()));
        w.ball.hold(0);
        w.players[0].native_tricks = Some(street_sim::NativeTricks {
            slots: [identity; 11],
        });
        // Start on the right hand: the primary side plays.
        w.players[0].gait_channel.hand = 1;
        // Dribbling (a fresh take standing still clears out instead).
        w.players[0].dribble = 1;
        use street_sim::button::{L1, L2, R1, R2, TRICK};
        w.step(&[street_sim::Input::default().with(TRICK | L1 | R1 | L2 | R2)]);
        assert!(matches!(
            w.players[0].action,
            street_sim::player::Action::Trick { .. }
        ));
        assert_eq!(w.players[0].gait_channel.trick_side, 0);
        let mut hands = vec![w.players[0].gait_channel.hand];
        while matches!(
            w.players[0].action,
            street_sim::player::Action::Trick { .. }
        ) {
            w.step(&[street_sim::Input::default()]);
            hands.push(w.players[0].gait_channel.hand);
        }
        // It ends on the left hand, having been on the right.
        assert_eq!(*hands.last().unwrap(), 2);
        assert!(hands.contains(&1));
    }

    #[test]
    fn every_court_plays_on_the_original_rims() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let mut iso = Iso::open(path).unwrap();
        for id in [
            "CENTER", "FOREST", "FOSTER", "INDOOR", "MOSSWD", "NYCAGE", "PHILLY", "RUCKER",
            "RUCKRC", "SEATTL", "SOLHOL",
        ] {
            let court = court::load(&mut iso, id).unwrap();
            assert_eq!(hoops(&court).unwrap(), Hoop::native(), "{id}");
        }
        // The ring's centre is 36.75 out, 1.25 in from the board at 38:
        // team 0's at +x.
        assert_eq!(Hoop::native()[0].rim[0], 38.0 - 1.25);
    }

    #[test]
    fn match_assets_include_native_gait_and_every_referenced_clip() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let loaded = load_clips(&mut Iso::open(path).unwrap()).unwrap();
        assert_eq!(loaded.gait.records.len(), street_assets::gait::RECORD_COUNT);
        for record in &loaded.gait.records {
            for row in record.rows {
                let name = loaded.timeline.animations[usize::from(row)]
                    .name
                    .as_ref()
                    .unwrap();
                assert!(loaded.clips.get(name).is_some(), "gait row {row}: {name}");
            }
        }
        for row in &loaded.ordinary.animation_rows {
            let name = loaded.timeline.animations[usize::from(*row)]
                .name
                .as_ref()
                .unwrap();
            let clip = loaded.clips.get(name).unwrap();
            assert!(clip.frames() > 0);
        }
    }

    #[test]
    fn a_rematch_is_built_on_the_generator_the_last_match_left() {
        use bevy::ecs::system::RunSystemOnce;
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(Iso::open(path).unwrap()).unwrap();
        let setup = MatchSetup::new(&disc, 0, 1);
        let mut world = World::new();
        world.insert_resource(disc);
        world.insert_resource(setup);
        world.insert_resource(crate::session::Session::pinned(2002));
        world.insert_resource(Time::<Virtual>::default());
        // The game's construction on the session's generator
        // (`World::from_generator`): its pair, and the match runs on from
        // the state after it.
        let built = |rng: street_sim::Rng| {
            street_sim::World::from_generator(Hoop::native(), Vec::new(), Rules::default(), rng)
        };
        load_match(&mut world).unwrap();
        let first = built(street_sim::Rng::new(2002));
        let sim = &world.resource::<game::Sim>().world;
        assert_eq!(sim.stage_bounds, first.stage_bounds);
        let mut after = first.generator().clone();
        assert!(
            (0..64).any(|_| {
                let same = &after == sim.generator();
                after.next_u32();
                same
            }),
            "the loaded match runs on from its construction"
        );
        // The match plays on and draws; leaving it hands its generator to
        // the session, and the rematch is built on that.
        for _ in 0..1_200 {
            let mut sim = world.resource_mut::<game::Sim>();
            let n = sim.world.players.len();
            sim.world.step(&vec![Input::default(); n]);
        }
        let left = world.resource::<game::Sim>().world.generator().clone();
        assert_ne!(&left, first.generator());
        world.run_system_once(game::leave).unwrap();
        assert_eq!(
            world.resource::<crate::session::Session>().generator(),
            left
        );
        load_match(&mut world).unwrap();
        let rematch = built(left);
        assert_eq!(
            world.resource::<game::Sim>().world.stage_bounds,
            rematch.stage_bounds
        );
    }

    #[test]
    fn a_loaded_match_opens_with_the_referees_toss() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(Iso::open(path).unwrap()).unwrap();
        let setup = MatchSetup::new(&disc, 0, 1);
        let mut world = World::new();
        world.insert_resource(disc);
        world.insert_resource(setup);
        load_match(&mut world).unwrap();
        let sim = &world.resource::<game::Sim>().world;
        // FUN_00141fd8: the players at their tip-off spots, the ball with
        // the referee, the toss on its way; no inbound.
        assert_eq!(
            sim.opening_toss,
            street_sim::OpeningToss::Waiting(street_sim::toss::WAIT_UPDATES)
        );
        assert_eq!(sim.ball.state, street_sim::BallState::Loose);
        assert_eq!(sim.ball.position, street_sim::toss::RELEASE);
        // Each jumper in the half his team defends (team 0 attacks +x).
        assert_eq!(sim.players[0].position, [-3.25, 0.0]);
        assert_eq!(sim.players[3].position, [3.25, 0.0]);
        assert_eq!(sim.phase, street_sim::Phase::Live);
        // Every player weighs his record's attribute 9.
        let setup = world.resource::<MatchSetup>();
        let disc = world.resource::<Disc>();
        for (i, record) in setup.lineups.iter().flatten().enumerate() {
            assert_eq!(
                sim.players[i].weight,
                disc.players[*record].weight_lb as u16
            );
        }
        // His model (FUN_003168b8's 0x3a1 and 0x3a2) is in the scene.
        let referee = world
            .resource::<Scene>()
            .referee
            .as_ref()
            .expect("the referee");
        assert_eq!(referee.models.len(), 1);
        assert!(!referee.models[0].meshes.is_empty());
        assert!(!referee.textures.is_empty());
        assert_eq!(referee.skeleton.bones.len(), 43);
    }

    #[test]
    fn every_machine_names_the_teams_as_a_match_alone_does() {
        // The Rundown draws each side's city over its nickname
        // (`FUN_002c7b48`'s elements 2 and 10). The creator plays the match
        // it loaded alone and sends its records' city and nickname in the
        // looks; a joiner names the sides from those, also for a created
        // team, which its own disc only knows as the base team standing in
        // for the logo.
        let team = |index: usize, city: &str, nickname: &str| db::Team {
            index,
            name: format!("{city} {nickname}"),
            city: city.into(),
            nickname: nickname.into(),
            roster: Vec::new(),
            logo: index as i16,
        };
        let creator = vec![
            team(0, "Philadelphia", "76ers"),
            team(1, "Los Angeles", "Lakers"),
            team(2, "The", "Rucker Crew"),
        ];
        // The joiner's disc: the created team 2 isn't there; the setup
        // names its logo's base team, 0.
        let joiner = vec![
            team(0, "Philadelphia", "76ers"),
            team(1, "Los Angeles", "Lakers"),
        ];
        for (sides, theirs) in [([1, 0], [1, 0]), ([2, 1], [0, 1])] {
            let alone = team_names(None, &creator, sides);
            let looks = crate::net_looks::NetLooks {
                teams: sides.map(|t| (creator[t].city.clone(), creator[t].nickname.clone())),
                ..Default::default()
            };
            let looks = crate::net_looks::NetLooks::decode(&looks.encode()).unwrap();
            let joined = team_names(Some(&looks), &joiner, theirs);
            assert_eq!((&joined.0, &joined.1), (&alone.0, &alone.1));
            // The city and the name apart, as two lines.
            assert!(
                alone
                    .0
                    .iter()
                    .all(|n| !n.contains(' ') || n == "Rucker Crew")
            );
            assert!(alone.1.iter().all(|c| !c.is_empty()));
        }
        let alone = team_names(None, &creator, [1, 0]);
        assert_eq!(alone.0, ["Lakers", "76ers"]);
        assert_eq!(alone.1, ["Los Angeles", "Philadelphia"]);
    }
}
