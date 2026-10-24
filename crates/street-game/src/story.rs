//! The progression save (Be A Legend and the League Challenge), kept on disk
//! ("Saves (to disk, not memory cards)", per the roadmap), and what the
//! menus and the match need from it: a run selected from the story, or an
//! League Challenge match.

use std::path::PathBuf;

use bevy::prelude::*;
use street_sim::bracket::Entrant;
pub use street_sim::story::{
    EVENTS, EventKind, MAX_RATING, REGION_MATCHES, REGION_NAMES, REGION_REWARDS, SKILLS, Story,
};
use street_sim::{Difficulty, Rules, story};

use crate::setup::{Disc, MatchSetup};

/// A crew has three starters, like every match's side.
pub const CREW_SIZE: usize = 3;

/// The game's save: the story state and the file it lives in.
#[derive(Resource)]
pub struct Save {
    pub path: PathBuf,
    pub story: story::Story,
    /// Failed to write, for the menu's message line.
    pub last_error: Option<String>,
}

impl Save {
    /// Reads `path`, or a fresh story if it is not there (a new game).
    pub fn load(path: PathBuf) -> Self {
        let mut story = std::fs::read(&path)
            .ok()
            .and_then(|bytes| story::Story::read(&bytes))
            .unwrap_or_default();
        story.trophy_rewards();
        Self {
            path,
            story,
            last_error: None,
        }
    }

    /// A change to the game data, kept in memory: the file stands for the
    /// memory card and is written only where the original writes the card,
    /// with the card's own dialogs ([`crate::card`]); until then the change
    /// lives as the original's does in its game data in RAM (`0x4bbf10`).
    pub fn store(&mut self) {
        self.story.trophy_rewards();
    }

    /// Writes the story out (the card's Saving); keeps the error for the
    /// card's "Save failed." otherwise.
    pub fn store_now(&mut self) {
        self.story.trophy_rewards();
        if let Some(dir) = self.path.parent()
            && !dir.as_os_str().is_empty()
        {
            let _ = std::fs::create_dir_all(dir);
        }
        match std::fs::write(&self.path, self.story.write()) {
            Ok(()) => self.last_error = None,
            Err(e) => self.last_error = Some(format!("could not save: {e}")),
        }
    }
}

/// What the played match counts toward, if it came out of the story.
#[derive(Clone, Copy, Debug)]
pub enum Run {
    /// A story event's game.
    StoryEvent(usize),
    /// An League Challenge match: region and match index.
    League(usize, usize),
    /// A Street School lesson's practice game.
    School(usize),
}

#[derive(Resource, Default)]
pub struct Running(pub Option<Run>);

/// The selected league person must participate locally on their team's side
/// ([`CAREER_SIDE`]). An empty seat list's ordinary match fallback does not
/// select a profile.
pub fn league_selected_user_eligible(setup: &MatchSetup) -> bool {
    !setup.demo && !setup.online && matches!(setup.seats.as_slice(), [(_, CAREER_SIDE)])
}

/// The production name-selection route and postgame path share this binding.
/// A fresh guest lives only until the next mode entry.
pub fn begin_league_for_setup(progress: &mut Story, setup: &MatchSetup) {
    let user = setup
        .users
        .first()
        .and_then(Option::as_deref)
        .and_then(|name| progress.records.user(name));
    progress.select_league_user(user);
    progress.restart_league();
}

pub fn play_league_for_setup(
    progress: &mut Story,
    setup: &MatchSetup,
    region: usize,
    index: usize,
    outcome: &story::MatchOutcome,
) -> Vec<story::Notice> {
    if !league_selected_user_eligible(setup) {
        return Vec::new();
    }
    let selected = setup
        .users
        .first()
        .and_then(Option::as_deref)
        .and_then(|name| progress.records.user(name))
        .map(|index| progress.records.users[index].name.clone());
    if !progress.league_profile_selected || progress.league_user != selected {
        begin_league_for_setup(progress, setup);
    }
    progress.play_league(region, index, outcome)
}

/// `FUN_002453e8`'s Alternate Ball arm substitutes the normal ball for
/// the last undefeated opponent in an uncleared League Challenge region.
/// A staged league run supplies the original pending-challenge/user gates;
/// `FUN_001ee548` treats the current opponent as already done.
pub fn match_ball_suffix(
    cheats: &crate::cheats::Settings,
    run: Option<Run>,
    progress: &Story,
) -> char {
    if cheats.mode(0x10) == Some(2)
        && let Some(Run::League(region, current)) = run
        && region < 5
        && let Some(teams) = REGION_MATCHES.get(region)
        && current <= teams.len()
        && progress.region_cleared & (1 << region) == 0
        && (0..teams.len()).all(|index| index == current || progress.league_done(region, index))
    {
        'a'
    } else {
        cheats.ball_model_suffix()
    }
}

/// The original mode-specific outer weight used by the meter constructor.
pub fn meter_weight(
    run: Option<Run>,
    story: &Story,
    disc: &Disc,
    setup: &MatchSetup,
) -> Option<f32> {
    match run? {
        Run::League(region, current) => {
            let count = REGION_MATCHES.get(region)?.len();
            let completed = (0..count).all(|i| story.league_done(region, i));
            let opponents = (0..count)
                .filter(|&i| i == current || story.league_done(region, i))
                .count();
            Some(street_sim::meter::league_weight(
                setup.rules.difficulty,
                completed,
                opponents,
            ))
        }
        Run::StoryEvent(event) => {
            let ev = EVENTS.get(event)?;
            let tier = story
                .crew
                .first()
                .and_then(|&r| disc.players.get(usize::from(r)))
                .filter(|p| p.team == 37)
                .and_then(|p| p.fields.get(15))
                .copied()
                .unwrap_or(-1)
                .max(0)
                / 11;
            let (round, rounds) = story
                .brackets
                .iter()
                .find(|(e, _)| usize::from(*e) == event)
                .and_then(|(_, b)| b.current().map(|(r, _, _)| (r, b.rounds())))
                .unwrap_or((0, 0));
            Some(street_sim::meter::legend_weight(
                tier as u8,
                ev.level,
                round,
                rounds,
                setup.teams[OPPONENT_SIDE] > 39,
            ))
        }
        Run::School(_) => None,
    }
}

/// The save's file name.
pub const SAVE_FILE: &str = "street_save.dat";
/// The folder under the user's Documents the game keeps its save in.
pub const SAVE_FOLDER: &str = "Streetball Revival";

/// The default save location (Vj, 2026-10-21): the PC's Documents folder,
/// `Documents\Streetball Revival\street_save.dat`; where there is no
/// Documents folder, the folder the game runs from.
pub fn default_path() -> PathBuf {
    documents()
        .map(|d| d.join(SAVE_FOLDER).join(SAVE_FILE))
        .or_else(|| std::env::current_dir().ok().map(|d| d.join(SAVE_FILE)))
        .unwrap_or_else(|| PathBuf::from(SAVE_FILE))
}

/// The user's Documents folder: `%USERPROFILE%\Documents` (or OneDrive's
/// when that is where Windows keeps it), `$HOME/Documents` elsewhere.
fn documents() -> Option<PathBuf> {
    let home = std::env::var_os("USERPROFILE")
        .or_else(|| std::env::var_os("HOME"))
        .map(PathBuf::from)?;
    let candidates = [
        home.join("Documents"),
        home.join("OneDrive").join("Documents"),
    ];
    candidates.into_iter().find(|d| d.is_dir())
}

/// Brings an earlier save to `to` when `to` has none yet: the first of
/// `from` that exists is copied (the old file stays where it was). Returns
/// the file it came from.
pub fn adopt_earlier_save(to: &std::path::Path, from: &[PathBuf]) -> Option<PathBuf> {
    if to.exists() {
        return None;
    }
    let old = from.iter().find(|p| p.is_file() && p.as_path() != to)?;
    if let Some(dir) = to.parent() {
        std::fs::create_dir_all(dir).ok()?;
    }
    std::fs::copy(old, to).ok()?;
    Some(old.clone())
}

/// Stage the match the event asks for: your crew against the event's
/// opponents, on its court, with its rules (`FUN_001e91a0`).
///
/// The opponents are `FUN_001dbe30`'s three in Be A Legend's mode
/// ([`crate::lineup`]): the session is reseeded from the timer for the
/// side (`FUN_001dbb20`), and a tournament's opponent in your next bracket
/// match (a boss crew, or the generic crews, team 0x24) is drawn from the
/// bracket's seed; a pick-up and a street challenge play team 0x24, drawn
/// from the timer's seed by the court.
pub fn prepare(
    ev: usize,
    story: &story::Story,
    disc: &crate::setup::Disc,
    setup: &mut MatchSetup,
    session: &mut crate::session::Session,
) -> Result<(), String> {
    if story.crew.len() < CREW_SIZE {
        return Err("pick your crew first".into());
    }
    let event = &story::EVENTS[ev];
    setup.rules = story::event_rules(ev, &setup.rules);
    setup.court = court_of(disc, event.court)?;
    seat_crew(setup, story);
    let (team, seed, round) = if event.kind == story::EventKind::Tournament {
        let b = story.tournament(ev);
        let (round, row, side) = b.current().ok_or("this tournament is over")?;
        let opponent = b.rows[row].sides[1 - side];
        let base = bracket_seed(&b);
        // `FUN_001e82f8`, your round; in double elimination the opponent's
        // entry past yours (`FUN_001e86e0`): the boss's is the last.
        let round = if b.kind == 2 {
            match opponent {
                Entrant::Crew(n) => n.wrapping_sub(base),
                _ => 3,
            }
        } else {
            round as u32
        };
        let team = match opponent {
            Entrant::Boss(team) => usize::from(team),
            Entrant::Crew(_) => GENERIC_TEAM,
            _ => return Err("no opponent yet".into()),
        };
        (team, Some(base), round)
    } else {
        (GENERIC_TEAM, None, 0)
    };
    setup.teams[OPPONENT_SIDE] = team;
    session.team_set(setup, OPPONENT_SIDE);
    let book = crate::lineup::Book::of(disc);
    let crew: Vec<usize> = story.crew.iter().map(|&r| usize::from(r)).collect();
    let star = crew.first().copied();
    let created: Vec<usize> = story
        .ballers
        .iter()
        .filter(|b| !b.career_only() && Some(b.record()) != star)
        .map(|b| b.record())
        .collect();
    let unlocked = crate::lineup::unlocked(disc, story);
    let cleared = |e: usize| story.cleared(e);
    let mode = crate::lineup::Mode::Legend(crate::lineup::Career {
        event: ev,
        crew: &crew,
        created: &created,
        unlocked: &unlocked,
        cleared: &cleared,
        round,
    });
    setup.lineups[OPPONENT_SIDE] = session.choose(&book, &mode, team, [None; 3], seed);
    setup.teams[CAREER_SIDE] = 37; // Create Player Team: your crew's own marquee
    Ok(())
}

/// The generic ballers' team (0x24), which a story's pick-ups, street
/// challenges and tournament crews play as.
const GENERIC_TEAM: usize = 36;

/// A bracket's seed: your entry's number (`FUN_001e9888` counts the
/// crews up from it, the first beside you one past it), which every game
/// of the tournament seeds `FUN_001dbe30` with (`FUN_001e8790(.., 0, 0,
/// 0)`). A one-series bracket against a boss keeps it as its generator,
/// which only the computer's own matches turn.
fn bracket_seed(b: &street_sim::bracket::Bracket) -> u32 {
    let first = b.round_rows(0).first().map_or(0, |&r| usize::from(r));
    match b.rows[first].sides[1] {
        Entrant::Crew(n) => n.wrapping_sub(1),
        _ => b.rng,
    }
}

/// A bracket entrant's name: your crew by its star, a boss crew by its
/// team, a generic crew as the bracket's own page names it, "Team %1d"
/// (`common.txt` 42) numbered from the bracket's first crew
/// ([`crate::bracket_view::entrant_name`]); its three are drawn when it is
/// met (`FUN_001dbe30`, [`crate::lineup`]).
pub fn entrant_name(
    e: Entrant,
    b: &street_sim::bracket::Bracket,
    story: &story::Story,
    disc: &crate::setup::Disc,
) -> String {
    let star = |r: usize| disc.players.get(r).map(|p| p.name()).unwrap_or_default();
    match e {
        Entrant::Empty => "--".into(),
        Entrant::You => match story.crew.first() {
            Some(&r) => format!("{}'s Crew (you)", star(usize::from(r))),
            None => "Your Crew".into(),
        },
        Entrant::Boss(t) => disc
            .teams
            .get(usize::from(t))
            .map_or_else(|| format!("Team {t}"), |t| t.full_name()),
        Entrant::Crew(n) => {
            let first = b
                .rows
                .iter()
                .flat_map(|m| m.sides)
                .filter_map(|e| match e {
                    Entrant::Crew(n) => Some(n),
                    _ => None,
                })
                .min()
                .unwrap_or(n);
            format!("Team {}", n.wrapping_sub(first) + 1)
        }
    }
}

/// The bracket page: Play (or Start), Forfeit and Back, how it stands,
/// then every round's matches with their series scores.
pub fn bracket_items(story: &story::Story, disc: &crate::setup::Disc, ev: usize) -> Vec<String> {
    // The one under way or just finished, else the one a start would make.
    let b = story
        .brackets
        .iter()
        .find(|(e, _)| usize::from(*e) == ev)
        .map(|(_, b)| b.clone())
        .unwrap_or_else(|| story.tournament(ev));
    let fresh = b.out() || b.won() || b.rows.iter().all(|m| m.wins == [0, 0]);
    let mut out = vec![
        if fresh {
            "Start Tournament".into()
        } else {
            "Play Next Game".into()
        },
        "Forfeit Tournament".into(),
        "Back".into(),
    ];
    if b.won() {
        out.push("CHAMPION".into());
    } else if b.out() {
        out.push("Knocked out".into());
    }
    for round in 0..b.rounds() {
        out.push(street_sim::bracket::round_name(b.kind, round).to_owned());
        for &row in b.round_rows(round) {
            let row = usize::from(row);
            let [a, c] = b.rows[row].sides;
            let score = b.score(row);
            let score = if score == [0, 0] {
                String::new()
            } else {
                format!("  {}-{}", score[0], score[1])
            };
            out.push(format!(
                "   {}  vs  {}{score}",
                entrant_name(a, &b, story, disc),
                entrant_name(c, &b, story, disc)
            ));
        }
    }
    out
}
fn court_of(disc: &crate::setup::Disc, court: u16) -> Result<String, String> {
    disc.courts
        .get(usize::from(court))
        .map(|c| c.id.clone())
        .ok_or_else(|| format!("no such court #{court}"))
}

/// The side the person's team plays in Be A Legend and the League Challenge:
/// those modes give it the setup's slot 0 (`FUN_001dbb20(setup, 0, ..)`:
/// the career's team, `FUN_001e91a0`; the challenge's, `FUN_0026f958`),
/// which plays the game's side 1 (`FUN_00185138` reads game side g's
/// players from slot g ^ 1; save states 4 and 10: game+0x188 the career's
/// team 49, the person on actor 3, the crew attacking -x first). The
/// opponents, slot 1, play side 0.
pub const CAREER_SIDE: usize = 1;
/// The other side ([`CAREER_SIDE`]).
pub const OPPONENT_SIDE: usize = 0;

/// Seat the crew: the career's side ([`CAREER_SIDE`]) becomes the three
/// crew records. The star is first (your baller).
pub(crate) fn seat_crew(setup: &mut MatchSetup, story: &story::Story) {
    let crew: Vec<usize> = story
        .crew
        .iter()
        .map(|&r| usize::from(r))
        .take(CREW_SIZE)
        .collect();
    if crew.len() == CREW_SIZE {
        setup.lineups[CAREER_SIDE] = crew.try_into().expect("three crew records");
        // The crew plays as the create-a-player team.
        setup.teams[CAREER_SIDE] = 37;
    }
}

/// What a match can be set up with for the League Challenge: the challenge's
/// team (Choose Team, class 0x19, kept as the user's `+0x4a`) against an
/// league team on the region's court, Normal rules. The team's first three
/// stand until the person picks three of its players on Choose Players
/// (mode 4); the opponent plays its first three (`FUN_001dbe30`).
pub fn prepare_league(
    region: usize,
    index: usize,
    story: &story::Story,
    disc: &crate::setup::Disc,
    setup: &mut MatchSetup,
) -> Result<(), String> {
    let matches = story::REGION_MATCHES
        .get(region)
        .ok_or("no such league region")?;
    let opponent = Story::league_opponent(region, index).ok_or("no such league opponent")?;
    if index == matches.len() && !story.league_boss_open(region) {
        return Err("Beat every league team in this region to play its legends".into());
    }
    let team = story
        .league_team
        .map(usize::from)
        .filter(|&team| {
            disc.teams
                .get(team)
                .is_some_and(|t| t.roster.len() >= CREW_SIZE)
        })
        .ok_or("Choose a team for the League Challenge first")?;
    let opponent = usize::from(opponent);
    // Play It's constructor put the rules back (`FUN_002beef8` →
    // `FUN_001de450` → `FUN_001ded20`) and nothing on the League Challenge's
    // way sets them (`FUN_0026bdc8` sets the opponent and the court only):
    // Normal to 21 on the 24-second clock, the points table's type 0.
    setup.rules = Rules::from_setup(0, 2, 21, [0, 0], &setup.rules);
    setup.court = court_of(disc, story::REGION_REWARDS[region].1)?;
    setup.lineups[CAREER_SIDE] = std::array::from_fn(|k| disc.teams[team].roster[k]);
    setup.teams[CAREER_SIDE] = team;
    let roster = disc.teams[opponent]
        .roster
        .get(..CREW_SIZE)
        .ok_or("the league opponent has no roster to play")?;
    setup.teams[OPPONENT_SIDE] = opponent;
    setup.lineups[OPPONENT_SIDE] = roster.try_into().expect("three opponent records");
    Ok(())
}

/// "Choose Baller": the ten careers, each its baller and how far along
/// (`storymainmenu.txt`: "%1d/%2d", "Complete", "<empty>"), then Back.
pub fn career_items(progress: &story::Story, disc: &Disc) -> Vec<String> {
    let mut out: Vec<String> = (0..story::CAREER_SLOTS)
        .map(|slot| match progress.career_at(slot) {
            Some(c) => {
                let name = c
                    .crew
                    .first()
                    .and_then(|&r| disc.players.get(usize::from(r)))
                    .map_or_else(|| "Baller".to_owned(), |p| p.name());
                let (points, complete) = progress.career_standing(slot).unwrap_or_default();
                let state = if complete {
                    "Complete".to_owned()
                } else {
                    format!("{points}/{}", story::OVERALL_TARGET)
                };
                format!("{}. {name:<20} {state}", slot + 1)
            }
            None => format!("{}. <empty>", slot + 1),
        })
        .collect();
    out.push("Back".into());
    out
}

/// A new career's baller: the six create-a-player records, made or
/// premade (`storymainmenu.txt` 10), then Back.
pub fn new_career_items(progress: &story::Story, disc: &Disc) -> Vec<String> {
    let mut out: Vec<String> = (0..story::BALLER_SLOTS)
        .map(|slot| {
            let record = usize::from(story::BALLER_RECORDS) + slot;
            let made = progress.ballers.iter().any(|b| usize::from(b.slot) == slot);
            format!(
                "{:<20} {}",
                disc.players[record].name(),
                if made { "Created" } else { "Premade" }
            )
        })
        .collect();
    out.push("Back".into());
    out
}

/// A nickname earned (`givenickname.txt`): one of the nicknames that go
/// with the star's best skill (its rating with the Gym's bumps), and the
/// line the post-game shows.
pub fn give_nickname(progress: &mut story::Story, disc: &Disc) -> String {
    // Deterministic seed from the career counters: the original draws from
    // its global LCG (`FUN_003b42a0`), whose exact state depends on every
    // prior consumer and is not reconstructable — but the same career must
    // always earn the same nickname (the wall clock previously used here
    // made it random per run).
    let roll = progress.games.wrapping_mul(2_654_435_761)
        ^ progress.wins.wrapping_mul(40_503)
        ^ progress.losses.wrapping_mul(65_537)
        ^ (progress.reputation as u32).wrapping_mul(97);
    give_nickname_with_roll(progress, disc, roll)
}

/// `FUN_001e70e0` selects the largest skill bar and `FUN_001d2070`
/// shuffles the matching contiguous nickname rows before drawing a row.
/// The supplied value seeds the native LCG; the original global seed and
/// unrelated random consumers are not reconstructed here.
pub fn give_nickname_with_roll(progress: &mut story::Story, disc: &Disc, roll: u32) -> String {
    let Some(&star) = progress.crew.first() else {
        return String::new();
    };
    let Some(p) = disc.players.get(usize::from(star)) else {
        return String::new();
    };
    let best = nickname_skill(&p.ratings, &progress.upgrades);
    let mut random = crate::progression_trace::NativeRandom::new(roll);
    let Some(row) = crate::progression_trace::nickname_row(&disc.nicknames, best, || random.draw())
    else {
        return "You've earned a nickname!".into();
    };
    let name = &disc.nicknames[row].name;
    progress.nickname_id = row as i16;
    progress.nickname = true;
    format!("You've earned a nickname! People have begun to call you \"{name}\".")
}

pub fn nickname_skill(ratings: &[i16; 11], upgrades: &[i16; 9]) -> street_assets::db::Skill {
    let adjusted = std::array::from_fn(|index| {
        (i32::from(ratings[index]) + i32::from(*upgrades.get(index).unwrap_or(&0))).clamp(0, 99)
            as i16
    });
    crate::progression_trace::nickname_skill(&adjusted)
}

/// The original game locale's introduction and per-name flavor. The
/// database's populated names follow the flavor text's order.
pub fn nickname_announcement(
    progress: &story::Story,
    disc: &Disc,
    intro: &[String],
    flavor: &[String],
) -> String {
    let Some(index) = usize::try_from(progress.nickname_id)
        .ok()
        .filter(|_| progress.nickname)
    else {
        return String::new();
    };
    let Some(n) = disc.nicknames.get(index) else {
        return String::new();
    };
    let lead = intro
        .get(1)
        .map_or("You've earned a nickname!", String::as_str);
    let line = flavor
        .get(index)
        .map(String::as_str)
        .or_else(|| intro.get(2).map(String::as_str))
        .unwrap_or("People have begun to call you:");
    format!("{lead}\n{line}\n{}", n.name)
}

/// The star's nickname, if earned.
pub fn nickname_of(progress: &story::Story, disc: &Disc) -> Option<String> {
    progress.nickname.then_some(())?;
    let row = usize::try_from(progress.nickname_id).ok()?;
    disc.nicknames.get(row).map(|name| name.name.clone())
}

/// Keep the active created star's raw player fields consistent with the
/// career record. Original `FUN_001e70e0` copies these changes back through
/// `FUN_001d9190`; field 15 also feeds the Be A Legend meter weight.
pub fn apply_progression_fields(progress: &story::Story, disc: &mut Disc) {
    let Some(&star) = progress.crew.first() else {
        return;
    };
    let Some(player) = disc.players.get_mut(usize::from(star)) else {
        return;
    };
    if player.team != 37 {
        return;
    }
    if let Some(field) = player.fields.get_mut(15) {
        *field = progress.reputation;
    }
    if let Some(field) = player.fields.get_mut(6) {
        *field = if progress.nickname {
            progress.nickname_id
        } else {
            -1
        };
    }
}

/// The players the crew can draw from when the story has unlocked nothing
/// yet: the Generic Ballers (the disc's filler crew).
pub fn recruit_pool(disc: &Disc, story: &story::Story) -> Vec<u16> {
    let mut out = story.unlocked_players.clone();
    for &r in &disc.teams[36].roster {
        if !out.contains(&(r as u16)) {
            out.push(r as u16);
        }
    }
    out.sort_unstable();
    out
}

// ---------- menus: the lists the Be A Legend pages show ----------

/// One city's events that show on the map, in the map's order.
pub fn city_rows(_progress: &story::Story, city: usize) -> Vec<usize> {
    // Every court of the city has its row, a locked one dimmed
    // (`FUN_002a1bb0` lights the markers whose event is shown and open; the
    // cursor stops on the others too, their help saying what unlocks them),
    // in the markers' order, a row's first slot before its second.
    crate::map_view::rows(city)
        .into_iter()
        .flat_map(|slots| slots.into_iter().flatten())
        .collect()
}

/// The game's `%1s`/`%2s`/`%1d`/`%2d` filled in order.
fn fill(template: &str, args: &[String]) -> String {
    let mut out = template.to_owned();
    for (k, a) in args.iter().enumerate() {
        for kind in ["s", "d"] {
            out = out.replace(&format!("%{}{kind}", k + 1), a);
        }
    }
    out.replace("\\n", " ")
}

/// The reputation level's name (`reputation.txt` 1..5: level 0 is none
/// yet).
pub fn level_name(level: u8) -> &'static str {
    match level {
        0 => "None",
        1 => "Rookie",
        2 => "Baller",
        3 => "All Star",
        4 => "Superstar",
        _ => "Legend",
    }
}

/// The map's read-out (`mapscreen.txt` 2, 3, 5): overall progress against
/// its target, the win-loss record and the reputation.
pub fn map_status(progress: &story::Story, text: &dyn Fn(usize) -> Option<String>) -> String {
    let t = |i: usize, plain: &str| text(i).unwrap_or_else(|| plain.to_owned());
    let points = fill(
        &t(2, "Progress: %1d/%2d"),
        &[
            progress.overall_points().to_string(),
            story::OVERALL_TARGET.to_string(),
        ],
    );
    let record = fill(
        &t(3, "%1dW-%2dL"),
        &[progress.wins.to_string(), progress.losses.to_string()],
    );
    format!(
        "{points}   {record}   {}: {}",
        t(5, "Reputation"),
        level_name(progress.reputation_level())
    )
}

/// An event's help line on its city's page (`mapscreen.txt` 16..18): what
/// it is, or what unlocks it.
pub fn event_help(
    progress: &story::Story,
    disc: &Disc,
    ev: usize,
    text: &dyn Fn(usize) -> Option<String>,
) -> String {
    let t = |i: usize, plain: &str| text(i).unwrap_or_else(|| plain.to_owned());
    let court = |e: usize| {
        disc.courts
            .get(usize::from(EVENTS[e].court))
            .map_or_else(String::new, |c| c.name.clone())
    };
    let word = |e: usize| {
        let (i, plain) = story::kind_word(e);
        t(i, plain)
    };
    if progress.open(ev) {
        // The help's own kind words (`mapscreen.txt` 7..9: "Pick Up Game",
        // "Tournament", "Street Challenge", by the node's kind in
        // `FUN_002a2548`'s caller), not the rows' (19..23).
        let (i, plain) = match EVENTS[ev].kind {
            EventKind::Pickup => (7, "Pick Up Game"),
            EventKind::Tournament => (8, "Tournament"),
            EventKind::StreetChallenge => (9, "Street Challenge"),
        };
        return fill(&t(16, "Play a %1s at %2s."), &[t(i, plain), court(ev)]);
    }
    match EVENTS[ev].open {
        story::Gate::Clear(e) => {
            let e = usize::from(e);
            fill(&t(18, "Clear %1s %2s to unlock."), &[court(e), word(e)])
        }
        story::Gate::Points(n) => fill(
            &t(17, "Earn %1d Progress Points to unlock."),
            &[n.to_string()],
        ),
        story::Gate::Now => String::new(),
    }
}

/// "Event Unlocked": the line the map shows for a newly opened event
/// (`mapscreen.txt` 36..38).
pub fn unlocked_line(disc: &Disc, ev: usize) -> String {
    let court = disc
        .courts
        .get(usize::from(EVENTS[ev].court))
        .map_or_else(String::new, |c| c.name.clone());
    let city = story::CITIES[story::city_of(ev)];
    let (_, word) = story::kind_word(ev);
    if EVENTS[ev].kind == EventKind::Pickup {
        format!("EVENT UNLOCKED: {court} {word} games are now available for play in {city}.")
    } else {
        format!("EVENT UNLOCKED: The {court} {word} is now available for play in {city}.")
    }
}

/// An intro's title (`pickupintro.txt` 0, `tourneyintro.txt` 0,
/// `streetchallengeintro.txt` 0, or 1 "Be A Legend" for the '78 challenge:
/// `crate::event_intro::title_id`); the rule's name is a line of its own.
pub fn intro_title(disc: &Disc, ev: usize) -> String {
    let row = &EVENTS[ev];
    let id = crate::event_intro::title_id(ev);
    let (texts, plain) = match row.kind {
        EventKind::Pickup => (&disc.intros[0], "Pick Up Game"),
        EventKind::Tournament => (&disc.intros[1], "Tournament"),
        EventKind::StreetChallenge => (&disc.intros[2], "Street Challenge"),
    };
    let at = usize::from(id & 0xff);
    texts
        .get(at)
        .filter(|t| t.as_str() != "n/a" && !t.is_empty())
        .cloned()
        .unwrap_or_else(|| plain.to_owned())
}

/// One city's rows: its events, then Back.
pub fn city_items(progress: &story::Story, disc: &Disc, city: usize) -> Vec<String> {
    let mut list: Vec<String> = city_rows(progress, city)
        .into_iter()
        .map(|ev| event_row(progress, disc, ev))
        .collect();
    // Back is Triangle, not a row (the page's prompts are Edit Baller and
    // Play Game).
    if list.is_empty() {
        list.push(String::new());
    }
    list
}

/// An event's row: what it is, where, and how far along.
fn event_row(progress: &story::Story, disc: &Disc, ev: usize) -> String {
    let row = &EVENTS[ev];
    let court = &disc
        .courts
        .get(usize::from(row.court))
        .map_or(row.court.to_string(), |c| c.name.clone());
    let (_, word) = story::kind_word(ev);
    let kind: String = match row.kind {
        EventKind::Pickup if progress.cleared(ev) => "won".into(),
        EventKind::Pickup => format!("PPs {} / {}", progress.progress[ev], row.target.max(0)),
        _ if progress.cleared(ev) => "won".into(),
        _ => format!("{} PPs", row.target.max(0)),
    };
    // A tournament under way: the round you're in.
    let kind = match progress
        .brackets
        .iter()
        .find(|(e, _)| usize::from(*e) == ev)
        .and_then(|(_, b)| b.current().map(|c| (b.kind, c.0)))
    {
        Some((k, round)) if row.kind == EventKind::Tournament => {
            format!("{kind}, {}", street_sim::bracket::round_name(k, round))
        }
        _ => kind,
    };
    let locked = if progress.open(ev) { "" } else { "  Locked" };
    format!("{word}: {court}  {kind}{locked}")
}

/// The map's items: the crew, the Gym, then the six cities with how many
/// of their events are won.
pub fn hub_items(progress: &story::Story, disc: &Disc) -> Vec<String> {
    let crew = if progress.crew.is_empty() {
        "-- pick your crew --".to_owned()
    } else {
        let mut names: Vec<String> = progress
            .crew
            .iter()
            .take(CREW_SIZE)
            .map(|&r| disc.players[r as usize].name())
            .collect();
        // The star's nickname beside the name.
        if let (Some(first), Some(nick)) = (names.first_mut(), nickname_of(progress, disc)) {
            *first = format!("{first} \"{nick}\"");
        }
        let starters = names.join(", ");
        match progress.crew.len().saturating_sub(CREW_SIZE) {
            0 => starters,
            bench => format!("{starters} (+{bench} on the bench)"),
        }
    };
    let level = level_name(progress.reputation_level());
    let mut list = vec![
        format!("Crew: {crew}"),
        format!(
            "The Gym  {} development points, {level}",
            progress.development_points,
        ),
    ];
    for (c, name) in story::CITIES.iter().enumerate() {
        let shown = city_rows(progress, c);
        if shown.is_empty() {
            list.push(format!("{name:<12}  Locked"));
        } else {
            let won = shown.iter().filter(|&&e| progress.cleared(e)).count();
            list.push(format!("{name:<12}  {won}/{} won", shown.len()));
        }
    }
    list
}

/// A player's row: name and height (narrow enough to leave the skills
/// panel clear).
fn player_row(disc: &Disc, record: u16) -> String {
    let p = &disc.players[usize::from(record)];
    format!(
        "{:<14} {}'{}\"",
        p.name(),
        p.height_in / 12,
        p.height_in % 12
    )
}

/// Your team: the three starters, then the bench.
pub fn team_items(progress: &story::Story, disc: &Disc) -> Vec<String> {
    progress
        .crew
        .iter()
        .enumerate()
        .map(|(k, &r)| {
            let role = match k {
                0 => "You    ",
                1 | 2 => "Starter",
                _ => "Bench  ",
            };
            format!("{role}  {}", player_row(disc, r))
        })
        .collect()
}

/// Update Team's rows at one step: Yes/No for the questions, the beaten
/// players (then Cancel Update), or your roster's five positions.
pub fn update_items(
    stage: crate::menu::UpdateStage,
    beaten: &[u16],
    pick: Option<u16>,
    progress: &story::Story,
    disc: &Disc,
) -> Vec<String> {
    use crate::menu::UpdateStage as S;
    match stage {
        S::Ask | S::Accept | S::Exit => vec!["Yes".into(), "No".into()],
        S::Pick => {
            let mut list: Vec<String> = beaten
                .iter()
                .map(|&r| {
                    let on = if progress.crew.contains(&r) {
                        "  (yours)"
                    } else {
                        ""
                    };
                    format!("{}{on}", player_row(disc, r))
                })
                .collect();
            list.push("Cancel Update".into());
            list
        }
        S::Slot => {
            let incoming = pick
                .map(|p| disc.players[usize::from(p)].name())
                .unwrap_or_default();
            let mut list: Vec<String> = progress
                .crew
                .iter()
                .enumerate()
                .map(|(k, &r)| format!("{}. {}", k + 1, player_row(disc, r)))
                .collect();
            if list.len() < street_sim::story::TEAM_SIZE {
                list.push(format!("{}. -- open --  ({incoming})", list.len() + 1));
            }
            list
        }
    }
}

/// The front end's songs' names in `bigradio.txt` (by
/// `music::FRONT_END_SONGS`' order; the list there also names two the menu
/// never plays).
const FRONT_END_TITLES: [usize; 9] = [10, 11, 12, 13, 14, 15, 16, 17, 20];
/// `0x467710`'s playlist order; catalog bits retain their native save order.
pub(crate) const FRONT_END_ORDER: [usize; 9] = [1, 0, 2, 3, 4, 5, 6, 7, 8];

pub(crate) fn front_end_title(index: usize) -> usize {
    FRONT_END_TITLES[index]
}

/// A press or left/right on Trax's row `at` (`songs` in-game songs).
pub fn trax_step(o: &mut street_sim::records::Settings, at: usize, songs: usize) {
    let songs = songs.min(street_sim::records::FRONT_END_BIT as usize);
    match at {
        0 => o.interactive = !o.interactive,
        1 => o.sequential = !o.sequential,
        2 => o.track_info = !o.track_info,
        k if o.interactive && k >= 3 && k < 3 + songs => o.songs_off ^= 1 << (k - 3),
        k if !o.interactive && k >= 3 && k < 3 + street_audio::music::FRONT_END_SONGS.len() => {
            o.songs_off ^=
                1 << (street_sim::records::FRONT_END_BIT as usize + FRONT_END_ORDER[k - 3]);
        }
        _ => {}
    }
}

/// The League Challenge's regions, with how much of each is played.
pub fn league_region_items(progress: &story::Story, disc: &Disc) -> Vec<String> {
    (0..REGION_MATCHES.len())
        .map(|r| {
            let need = REGION_MATCHES[r].len() + 1;
            let played = (0..need).filter(|&k| progress.league_done(r, k)).count();
            let tail = format!(
                "legends: {}",
                disc.teams[REGION_REWARDS[r].0 as usize].full_name()
            );
            format!("{:<13} {} / {}  {}", REGION_NAMES[r], played, need, tail)
        })
        .collect()
}

/// One region's matches, with the won ones ticked.
pub fn league_match_items(progress: &story::Story, region: usize, disc: &Disc) -> Vec<String> {
    REGION_MATCHES[region]
        .iter()
        .copied()
        .chain(std::iter::once(REGION_REWARDS[region].0))
        .enumerate()
        .map(|(k, team)| {
            let mark = if progress.league_done(region, k) {
                "[x]"
            } else {
                "[ ]"
            };
            let locked = if k == REGION_MATCHES[region].len() && !progress.league_boss_open(region)
            {
                " (locked until every regional league team is beaten)"
            } else {
                ""
            };
            format!("{mark} {}{locked}", disc.teams[team as usize].full_name())
        })
        .collect()
}

/// A mode's end board (`nbatcend.txt`, `bealegendend.txt`): the
/// congratulations, then for Be A Legend the star, the win-loss record, the
/// reputation level and the street challenges won (`FUN_00261c48`), then
/// Continue.
pub fn completed_items(
    progress: &story::Story,
    disc: &Disc,
    mode: usize,
    difficulty: Difficulty,
) -> Vec<String> {
    let mut out = Vec::new();
    if mode == 0 {
        out.push("You have completed League Challenge Mode.".into());
        let level = match difficulty {
            Difficulty::Easy => 0,
            Difficulty::Medium => 1,
            Difficulty::Hard => 2,
        };
        out.push(format!("{} Reward Points", story::LEAGUE_DONE_RP[level]));
    } else {
        out.push("You have completed Be A Legend Mode.".into());
        if let Some(&star) = progress.crew.first() {
            out.push(disc.players[usize::from(star)].name());
        }
        out.push(format!(
            "Win / Loss Record:  {} - {}",
            progress.wins, progress.losses
        ));
        // `reputation.txt` 1..5.
        let level = level_name(progress.reputation_level()).to_uppercase();
        out.push(format!("Reputation Level:  {level}"));
        let streets: Vec<usize> = (0..EVENTS.len())
            .filter(|&e| EVENTS[e].kind == EventKind::StreetChallenge)
            .collect();
        let won = streets.iter().filter(|&&e| progress.cleared(e)).count();
        out.push(format!(
            "Street Challenges:  {won} / {} COMPLETE",
            streets.len()
        ));
    }
    out.push("Continue".into());
    out
}

/// The card at `cursor` of a category's page (row 0 is the points).
pub fn reward_card(
    disc: &Disc,
    category: usize,
    cursor: usize,
) -> Option<&street_assets::db::Card> {
    disc.cards
        .iter()
        .filter(|c| c.category as usize == category && crate::rewards::in_shop(c))
        .nth(cursor.checked_sub(1)?)
}

/// The shop: buy the card at `cursor` of `category` (cursor 0 is the points
/// header; the last is Back).
pub fn reward_buy(
    story: &mut story::Story,
    disc: &Disc,
    category: usize,
    cursor: usize,
) -> Result<String, String> {
    let Some(card) = reward_card(disc, category, cursor) else {
        return Err("Back".into());
    };
    let records = crate::rewards::card_records(card);
    let grant = match card.category {
        crate::rewards::STREET_LEGENDS | crate::rewards::LEAGUE_LEGENDS if !records.is_empty() => {
            story::CardGrant::Player(records[0])
        }
        crate::rewards::COURTS => {
            crate::rewards::court_of(card).map_or(story::CardGrant::Thing, story::CardGrant::Court)
        }
        crate::rewards::DEVELOPMENT => {
            // The development rows take their price in Reward Points and
            // pay their amount in development points.
            return if story.spend_reward_points(card.cost as u32, card.index as u32) {
                Ok(format!("+{} development points", card.index))
            } else {
                Err(format!(
                    "needs {} Reward Points; the player has {}",
                    card.cost, story.reward_points
                ))
            };
        }
        _ => story::CardGrant::Thing,
    };
    if story.buy(card.id as u16, card.cost as u32, grant) {
        // Nelly's card brings the whole crew.
        for &r in records.iter().skip(1) {
            story.unlock_player(r);
        }
        // A bought code card shows the name and four face-button presses.
        let what = if card.category == crate::rewards::CODES {
            crate::rewards::cheat_info(&disc.common, card)
        } else {
            crate::rewards::card_name(&disc.poster, card)
        };
        Ok(format!("Unlocked: {what}"))
    } else if story.owned.contains(&(card.id as u16)) {
        Err("Already owned".into())
    } else {
        Err(format!(
            "needs {} Reward Points; the player has {}",
            card.cost, story.reward_points
        ))
    }
}

#[cfg(test)]
mod traced_ball_tests {
    use super::*;

    #[test]
    fn league_bosses_and_atlanta_use_their_disc_rosters_and_regional_courts() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut progress = Story::default();
        assert!(prepare_league(0, 0, &progress, &disc, &mut MatchSetup::new(&disc, 0, 1)).is_err());
        // The challenge's team (Choose Team, class 0x19): its first three
        // stand until Choose Players.
        progress.league_team = Some(16);
        let mut setup = MatchSetup::new(&disc, 0, 1);
        setup.rules.difficulty = Difficulty::Hard;
        setup.rules = Rules::mode(street_sim::Mode::TrickPointChallenge);
        setup.rules.difficulty = Difficulty::Hard;
        prepare_league(0, 0, &progress, &disc, &mut setup).unwrap();
        // Play It's put-back rules: Normal to 21, the 24-second clock.
        assert_eq!(setup.rules.game_type(), 0);
        assert_eq!(setup.rules.goal, street_sim::Goal::Points(21));
        assert_eq!(setup.rules.shot_clock, Some(24.0));
        assert_eq!(setup.teams[CAREER_SIDE], 16);
        assert_eq!(setup.lineups[CAREER_SIDE], disc.teams[16].roster[..3]);
        assert_eq!(league_region_items(&progress, &disc).len(), 5);
        assert_eq!(league_match_items(&progress, 0, &disc).len(), 7);
        let expected = [
            [169, 170, 171],
            [172, 173, 174],
            [175, 176, 177],
            [178, 179, 180],
            [181, 182, 183],
        ];
        for (region, teams) in REGION_MATCHES.iter().enumerate() {
            let boss = teams.len();
            assert!(prepare_league(region, boss, &progress, &disc, &mut setup).is_err());
            for index in 0..boss {
                progress.play_league(
                    region,
                    index,
                    &story::MatchOutcome {
                        won: true,
                        ..Default::default()
                    },
                );
            }
            prepare_league(region, boss, &progress, &disc, &mut setup).unwrap();
            assert_eq!(setup.teams[OPPONENT_SIDE], 31 + region);
            assert_eq!(setup.rules.difficulty, Difficulty::Hard);
            assert_eq!(setup.lineups[OPPONENT_SIDE], expected[region]);
            assert_eq!(
                setup.court,
                disc.courts[usize::from(REGION_REWARDS[region].1)].id
            );
            assert_eq!(
                meter_weight(Some(Run::League(region, boss)), &progress, &disc, &setup),
                Some(street_sim::meter::league_weight(
                    setup.rules.difficulty,
                    true,
                    teams.len()
                ))
            );
        }
        prepare_league(4, 4, &progress, &disc, &mut setup).unwrap();
        assert_eq!(setup.teams[OPPONENT_SIDE], 0);
        assert_eq!(setup.lineups[OPPONENT_SIDE], disc.teams[0].roster[..3]);
        assert!(prepare_league(5, 0, &progress, &disc, &mut setup).is_err());
    }

    #[test]
    fn trax_experiences_keep_separate_song_choices() {
        let mut settings = street_sim::records::Settings::default();
        // The Rucker Classic's song (in-game song 12) is the one left off.
        let rucker = 1 << street_sim::records::RUCKER_CLASSIC_SONG_BIT;
        let nelly = 1 << street_sim::records::NELLY_SONG_BIT;
        assert_eq!(settings.songs_off, rucker);
        trax_step(&mut settings, 3, 15);
        assert_eq!(settings.songs_off, 1 | rucker);
        trax_step(&mut settings, 0, 15);
        assert!(!settings.interactive);
        trax_step(&mut settings, 3, 15);
        assert_eq!(settings.songs_off, 1 | rucker | nelly);
        // Back and an out-of-range row cannot toggle either catalog.
        let before = settings;
        trax_step(&mut settings, 12, 15);
        trax_step(&mut settings, usize::MAX, 15);
        assert_eq!(settings, before);
        trax_step(&mut settings, 0, 15);
        trax_step(&mut settings, 3, 15);
        assert_eq!(settings.songs_off, rucker | nelly);
    }

    #[test]
    fn nicknames_are_deterministic_for_the_same_career() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let disc = crate::setup::Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let career = || story::Story {
            crew: vec![0],
            games: 42,
            wins: 30,
            losses: 12,
            reputation: 33,
            ..Default::default()
        };
        let (mut a, mut b) = (career(), career());
        let (line_a, line_b) = (
            super::give_nickname(&mut a, &disc),
            super::give_nickname(&mut b, &disc),
        );
        assert_eq!((&line_a, a.nickname_id), (&line_b, b.nickname_id));
        assert!(
            a.nickname && !a.nickname_id.is_negative() || line_a == "You've earned a nickname!"
        );
    }

    #[test]
    fn alternate_ball_normalizes_only_for_the_last_opponent_of_an_uncleared_region() {
        let mut cheats = crate::cheats::Settings::default();
        cheats.apply(crate::cheats::Cheat::AlternateBall);
        let mut story = Story::default();
        assert_eq!(
            match_ball_suffix(&cheats, Some(Run::League(0, 0)), &story),
            'e'
        );
        for index in 0..REGION_MATCHES[0].len() - 1 {
            story.league_played |= 1 << index;
        }
        let final_opponent = REGION_MATCHES[0].len() - 1;
        assert_eq!(
            match_ball_suffix(&cheats, Some(Run::League(0, final_opponent)), &story),
            'a'
        );
        story.league_played |= 1 << final_opponent;
        assert_eq!(
            match_ball_suffix(
                &cheats,
                Some(Run::League(0, REGION_MATCHES[0].len())),
                &story
            ),
            'a'
        );
        story.region_cleared |= 1;
        assert_eq!(
            match_ball_suffix(&cheats, Some(Run::League(0, final_opponent)), &story),
            'e'
        );
        assert_eq!(match_ball_suffix(&cheats, None, &story), 'e');
        cheats.apply(crate::cheats::Cheat::WomensBall);
        assert_eq!(
            match_ball_suffix(&cheats, Some(Run::League(0, final_opponent)), &story),
            'f'
        );
    }
}
