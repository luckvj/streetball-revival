//! The game: the original game's three on three, from the title screen and
//! menus to the final buzzer, with every model, animation and number read
//! from your own disc.
//!
//!     street-game [--iso <path>]                 the title screen and menus
//!     street-game --quick [--teams Lakers,76ers] [--court RUCKER] [--mode ...]
//!                 [--demo] [--speed <x>] [--screenshot <out.png> [--at <secs>]]
//!
//! Picking a match on the command line (`--teams`, `--court`, `--demo`,
//! `--screenshot` or `--quick`) skips the menus. See `usage.txt` for every
//! option and the controls.
//!
//! The game itself is `street-sim`, stepped at a fixed 60 Hz from the inputs;
//! this program reads the controls, fills in the computer players' inputs,
//! steps the simulation and draws it.

// Bevy systems take their resources as parameters and filter queries by
// marker types; both lints would flag ordinary Bevy code.
#![allow(clippy::too_many_arguments, clippy::type_complexity)]

mod ambience;
pub mod art;
pub mod attract;
pub mod audio;
pub mod ballers;
mod ballfx;
mod banner;
mod boards;
mod boot;
mod bracket_view;
mod cab_view;
mod card;
mod careers;
pub mod channel;
pub mod cheats;
mod choose;
mod choosecourt;
mod chooseteam;
mod clip;
mod completed_view;
mod controls;
mod create_team;
mod credits;
mod diagram;
mod drift;
mod edit_moves;
mod event_intro;
mod exit_ask;
pub mod game;
mod game_data;
mod grip;
mod head_turn;
mod help_view;
pub mod highscores;
pub mod hud;
mod intro;
mod join_view;
pub mod lineup;
pub mod loading;
mod loading_view;
mod lobby;
pub mod look;
mod map_view;
pub mod menu;
pub mod movie;
mod myrules;
mod net_looks;
pub mod online;
pub mod options;
mod options_view;
pub mod original_save;
pub mod pace;
pub mod pads;
mod pause;
mod portraits;
pub mod progression_trace;
mod reach;
pub mod referee;
mod region_view;
mod rematch_rules;
pub mod rewards;
mod rewards_view;
pub mod roster;
pub mod school;
mod school_menu;
mod school_prompt;
mod score_view;
mod screen_fx;
pub mod session;
pub mod setup;
pub mod sfx;
mod smooth;
pub mod song;
pub mod squeak;
pub mod staged;
pub mod story;
pub mod tape;
pub mod teams;
mod trails;
mod transition;
mod trax_view;
pub mod typing;
mod update_view;
mod users;
pub mod view;

use std::path::PathBuf;

use bevy::prelude::*;
use street_assets::iso::Iso;
use street_net::DEFAULT_PORT;
use street_render::{
    StreetRenderPlugin,
    app::{self, ScreenshotPlugin},
};
use street_sim::{Difficulty, Goal, Mode, Rules, TICK_HZ};

/// Which screen the game is on.
#[derive(States, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Screen {
    /// The boot's own loading screen and the memory card's check
    /// (`boot.rs`), before the boot's movies (the first title's
    /// transition).
    Boot,
    /// The title screen and menus.
    #[default]
    Menu,
    /// Reading the chosen match from the disc.
    Loading,
    /// Waiting for everyone in an online match ([`online`]).
    Lobby,
    Playing,
    /// The load back to the front end after a game (class 0x2f: its
    /// picture, [`loading::Back`], for the original's load).
    Returning,
}

struct Args {
    iso: Option<PathBuf>,
    court: String,
    /// Home and away team names.
    teams: [String; 2],
    /// `--seed`: the session's generator from this seed, not the clock.
    seed: Option<u32>,
    rules: Rules,
    demo: bool,
    /// Game speed relative to real time (for watching whole demo games).
    speed: f32,
    screenshot: Option<PathBuf>,
    /// Seconds into the game to take the screenshot.
    at: f32,
    /// `--every` / `--count`: a series of screenshots from `--at`.
    every: Option<f32>,
    count: usize,
    /// `--at-real`: a scripted run's screenshot by the real clock, which
    /// runs on through the screens' transitions (to catch one).
    at_real: bool,
    /// `--keys-real`: the scripted keys count real seconds, so a script can
    /// press while a transition runs (as a person can).
    keys_real: bool,
    /// `--at-transition N`: `--at` real seconds from the start of the
    /// run's Nth screen transition.
    at_transition: Option<usize>,
    /// `--events`: write the match's events with their update numbers here.
    events: Option<PathBuf>,
    /// Start a match straight away instead of at the title screen (set by
    /// `--quick` and by any option that picks a match).
    quick: bool,
    /// Where the story (Be A Legend) save lives.
    save: PathBuf,
    import_save: Option<PathBuf>,
    convert_save: bool,
    /// Key presses to replay, for checking the menus without a person:
    /// (seconds, key).
    keys: Vec<(f32, KeyCode, f32)>,
    /// `--intro` / `--no-intro`: play the boot movies or not.
    intro: Option<bool>,
    /// `--difficulty` was given: it wins over the saved option.
    difficulty_given: bool,
    /// `--widescreen off|on|wide`: the picture for this run.
    widescreen: Option<street_sim::records::Picture>,
    /// `--host` / `--join`: play online.
    online: Option<online::OnlineArgs>,
    /// `--full-meters` (Level 2) / `--level1`: the home team starts with a
    /// gamebreaker ready.
    full_meters: Option<bool>,
    /// `--level2-at x,z`: the away holder fires a Level 2 from there.
    level2_at: Option<[f32; 2]>,
    /// `--level2-script n`: the plan `--level2-at` fires.
    level2_script: Option<usize>,
    /// `--demo-delay`: the idle demo's wait, seconds, over the disc's
    /// `demo_delay` (for checking the demo without the 120 s).
    demo_delay: Option<f64>,
    play_by_play: bool,
    /// Player editor (`roster.rs`): `--roster-db`, `--roster`, `--hires`.
    roster_db: Option<PathBuf>,
    roster: Option<PathBuf>,
    hires: Option<PathBuf>,
    /// `--v3`, `--v1`: GameCube discs whose characters roster.json players
    /// can be drawn as.
    v3: Option<PathBuf>,
    v1: Option<PathBuf>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args {
        iso: None,
        court: "RUCKER".into(),
        teams: ["Lakers".into(), "76ers".into()],
        seed: None,
        rules: Rules::default(),
        demo: false,
        speed: 1.0,
        screenshot: None,
        at: 10.0,
        at_real: false,
        every: None,
        count: 1,
        keys_real: false,
        at_transition: None,
        events: None,
        quick: false,
        save: story::default_path(),
        import_save: None,
        convert_save: false,
        keys: Vec::new(),
        intro: None,
        difficulty_given: false,
        widescreen: None,
        online: None,
        full_meters: None,
        level2_at: None,
        level2_script: None,
        demo_delay: None,
        play_by_play: false,
        roster_db: None,
        roster: None,
        hires: None,
        v3: None,
        v1: None,
    };
    let mut menu = false;
    let mut port_given: Option<u16> = None;
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        let mut value = || it.next().ok_or(format!("{a} needs a value"));
        match a.as_str() {
            "--iso" => args.iso = Some(value()?.into()),
            "--court" => {
                args.court = value()?.to_ascii_uppercase();
                args.quick = true;
            }
            "--quick" => args.quick = true,
            // Start at the title screen even with match options given.
            "--menu" => menu = true,
            "--no-intro" => args.intro = Some(false),
            // The original's measured loading holds, not the fast ones.
            "--original-load-times" => pace::set_original(true),
            "--intro" => args.intro = Some(true),
            "--teams" => {
                let v = value()?;
                let Some((home, away)) = v.split_once(',') else {
                    return Err("--teams takes two names: home,away".into());
                };
                args.teams = [home.trim().into(), away.trim().into()];
                args.quick = true;
            }
            "--seed" => args.seed = Some(value()?.parse().map_err(|e| format!("--seed: {e}"))?),
            "--mode" => {
                let v = value()?.to_ascii_lowercase().replace(['-', '_', ' '], "");
                let mode = Mode::ALL
                    .into_iter()
                    .find(|m| {
                        m.name()
                            .to_ascii_lowercase()
                            .replace(' ', "")
                            .starts_with(&v)
                    })
                    .ok_or(format!("--mode: no mode called {v:?}"))?;
                // Keep settings given before --mode that the mode does not set.
                let (clock, difficulty) = (args.rules.shot_clock, args.rules.difficulty);
                args.rules = Rules::mode(mode);
                args.rules.shot_clock = clock;
                args.rules.difficulty = difficulty;
            }
            "--play-to" => {
                let n: u32 = value()?.parse().map_err(|e| format!("--play-to: {e}"))?;
                args.rules.goal = match args.rules.goal {
                    Goal::TrickPoints(_) => Goal::TrickPoints(n),
                    Goal::Gamebreaker(_) => Goal::Gamebreaker(n.clamp(1, 2) as u8),
                    Goal::Points(_) => Goal::Points(n),
                };
            }
            "--shot-clock" => {
                let v = value()?;
                args.rules.shot_clock = match v.as_str() {
                    "off" | "0" => None,
                    s => Some(s.parse().map_err(|e| format!("--shot-clock: {e}"))?),
                };
            }
            "--difficulty" => {
                args.difficulty_given = true;
                args.rules.difficulty = match value()?.to_ascii_lowercase().as_str() {
                    "easy" => Difficulty::Easy,
                    "medium" => Difficulty::Medium,
                    "hard" => Difficulty::Hard,
                    other => {
                        return Err(format!(
                            "--difficulty: {other:?} is not easy, medium or hard"
                        ));
                    }
                };
            }
            "--widescreen" => {
                use street_sim::records::Picture;
                args.widescreen = Some(match value()?.to_ascii_lowercase().as_str() {
                    "off" => Picture::Standard,
                    "on" => Picture::Anamorphic,
                    "wide" => Picture::Wide,
                    other => {
                        return Err(format!("--widescreen: {other:?} is not off, on or wide"));
                    }
                });
            }
            "--unlimited-turbo" => args.rules.unlimited_turbo = true,
            "--full-meters" => args.full_meters = Some(true),
            "--level1" => args.full_meters = Some(false),
            "--level2-at" => {
                let v = value()?;
                let at = v
                    .split_once(',')
                    .and_then(|(x, z)| Some([x.trim().parse().ok()?, z.trim().parse().ok()?]));
                args.level2_at =
                    Some(at.ok_or(format!("--level2-at: {v:?} is not x,z (court units)"))?);
                args.quick = true;
            }
            "--level2-script" => {
                let v = value()?;
                args.level2_script = Some(
                    v.parse()
                        .map_err(|_| format!("--level2-script: {v:?} is not a script"))?,
                );
            }
            "--play-by-play" => args.play_by_play = true,
            "--roster-db" => args.roster_db = Some(value()?.into()),
            "--roster" => args.roster = Some(value()?.into()),
            "--hires" => args.hires = Some(value()?.into()),
            "--v3" => args.v3 = Some(value()?.into()),
            "--v1" => args.v1 = Some(value()?.into()),
            "--demo" => {
                args.demo = true;
                args.quick = true;
            }
            "--speed" => args.speed = value()?.parse().map_err(|e| format!("--speed: {e}"))?,
            "--screenshot" => {
                args.screenshot = Some(value()?.into());
                args.quick = true;
            }
            "--at" => args.at = value()?.parse().map_err(|e| format!("--at: {e}"))?,
            "--every" => args.every = Some(value()?.parse().map_err(|e| format!("--every: {e}"))?),
            "--count" => args.count = value()?.parse().map_err(|e| format!("--count: {e}"))?,
            "--at-real" => args.at_real = true,
            "--keys-real" => args.keys_real = true,
            "--demo-delay" => {
                args.demo_delay = Some(value()?.parse().map_err(|e| format!("--demo-delay: {e}"))?)
            }
            "--at-transition" => {
                args.at_transition = Some(
                    value()?
                        .parse()
                        .map_err(|e| format!("--at-transition: {e}"))?,
                )
            }
            "--events" => args.events = Some(value()?.into()),
            "--keys" => args.keys = parse_keys(&value()?)?,
            "--save" => args.save = value()?.into(),
            "--import-save" => args.import_save = Some(value()?.into()),
            "--convert-save" => args.convert_save = true,
            "--host" => args.online.get_or_insert_with(Default::default).host = Some(DEFAULT_PORT),
            "--port" => {
                let port = value()?.parse().map_err(|e| format!("--port: {e}"))?;
                let o = args.online.get_or_insert_with(Default::default);
                if o.host.is_some() {
                    o.host = Some(port);
                }
                port_given = Some(port);
            }
            "--join" | "--create" => {
                let o = args.online.get_or_insert_with(Default::default);
                o.server = Some(value()?);
                o.create |= a == "--create";
            }
            "--match" => args.online.get_or_insert_with(Default::default).match_name = value()?,
            "--name" => args.online.get_or_insert_with(Default::default).name = value()?,
            "--side" => {
                args.online.get_or_insert_with(Default::default).side =
                    match value()?.to_ascii_lowercase().as_str() {
                        "home" => Some(0),
                        "away" => Some(1),
                        "any" => None,
                        other => return Err(format!("--side: {other:?} is not home, away or any")),
                    }
            }
            "--ready" => args.online.get_or_insert_with(Default::default).ready = true,
            "--bot" => args.online.get_or_insert_with(Default::default).bot = true,
            "--players" => {
                args.online.get_or_insert_with(Default::default).people = match value()?.as_str() {
                    "2" => 2,
                    "6" => 6,
                    v => return Err(format!("--players: 2 or 6, not {v}")),
                };
            }
            "-h" | "--help" => return Err(include_str!("usage.txt").into()),
            other => {
                return Err(format!(
                    "unknown argument {other:?}\n{}",
                    include_str!("usage.txt")
                ));
            }
        }
    }
    if menu {
        args.quick = false;
    }
    if let Some(o) = &mut args.online {
        if o.host.is_some() {
            o.host = port_given.or(o.host);
            // The host plays the home side unless it says otherwise.
            o.side = o.side.or(Some(0));
        } else if o.server.is_none() {
            return Err(
                "--match, --name, --side, --ready and --bot go with --host or --join".into(),
            );
        }
    }
    Ok(args)
}

fn main() {
    let args = match parse_args() {
        Ok(a) => a,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    if let Err(e) = run(args) {
        eprintln!("error: {e}");
        // Started by double-clicking: keep the window up to be read.
        if std::io::IsTerminal::is_terminal(&std::io::stdin()) && std::env::args().len() == 1 {
            eprintln!("\nPress Enter to close.");
            let _ = std::io::stdin().read_line(&mut String::new());
        }
        std::process::exit(1);
    }
}

fn run(args: Args) -> Result<(), Box<dyn std::error::Error>> {
    let iso_path = match args.iso {
        Some(p) => p,
        None => app::find_iso()?,
    };
    let iso = Iso::open(&iso_path).map_err(|e| format!("{}: {e}", iso_path.display()))?;
    // Player editor: a roster mod's tables replace the disc's before it loads.
    let env_path = |name| {
        std::env::var_os(name)
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
    };
    if let Some(dir) = args
        .roster_db
        .clone()
        .or_else(|| env_path("STREET_ROSTER_DB"))
    {
        eprintln!(
            "roster: tables from {}",
            roster::use_roster_db(&dir)?.display()
        );
    }
    // GameCube discs, before roster.json: their characters' table facts
    // start the records that name them.
    for (flag, path, var) in [
        ("--v3", args.v3.clone(), "STREET_V3_GC"),
        ("--v1", args.v1.clone(), "STREET_V1_GC"),
    ] {
        if let Some(path) = path.or_else(|| env_path(var)) {
            match street_assets::gc_discs::add(&path) {
                Ok((game, n)) => eprintln!(
                    "gamecube: {} ({n} characters) from {}",
                    game.prefix(),
                    path.display()
                ),
                Err(e) => eprintln!(
                    "{flag} {}: {e}; GameCube characters keep their Vol. 2 stand-ins",
                    path.display()
                ),
            }
        }
    }
    if let Some(path) = args.roster.clone().or_else(|| env_path("STREET_ROSTER")) {
        roster::use_roster(&path)?;
    }
    if let Some(dir) = args.hires.clone().or_else(|| env_path("STREET_HIRES")) {
        // Before the App runs: art and portraits load in Startup systems.
        match street_render::hires::set_pack(&dir) {
            Ok(pack) => eprintln!(
                "hires: {} of the pack's {} images used",
                pack.used().len(),
                pack.len()
            ),
            Err(e) => eprintln!("--hires {}: {e}", dir.display()),
        }
    }
    let mut disc = setup::Disc::load(iso)?;
    // The save lives in Documents now (Vj, 2026-10-21); a save the game
    // kept beside the disc or where it ran from comes along the first time.
    if args.save == story::default_path() {
        let beside = |d: Option<&std::path::Path>| d.map(|d| d.join(story::SAVE_FILE));
        let from: Vec<std::path::PathBuf> = [
            beside(std::env::current_dir().ok().as_deref()),
            beside(iso_path.parent()),
            beside(
                std::env::current_exe()
                    .ok()
                    .as_deref()
                    .and_then(std::path::Path::parent),
            ),
        ]
        .into_iter()
        .flatten()
        .collect();
        if let Some(old) = story::adopt_earlier_save(&args.save, &from) {
            eprintln!("save: copied {} to {}", old.display(), args.save.display());
        }
    }
    if let Some(dir) = args.save.parent()
        && !dir.as_os_str().is_empty()
    {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Some(path) = &args.import_save {
        let imported = original_save::import(&std::fs::read(path)?, &disc)?;
        imported.store_new(&args.save)?;
        for note in &imported.notes {
            eprintln!("save import: {note}");
        }
        eprintln!("Imported save written to {}", args.save.display());
    } else if args.convert_save {
        return Err("--convert-save needs --import-save".into());
    }
    if args.convert_save {
        return Ok(());
    }
    let clips = setup::load_clips(&mut disc.iso)?;
    let names = ballers::Names::load(&mut disc.iso);
    let fresh = !args.save.exists();
    let mut save = story::Save::load(args.save.clone());
    // A new profile's high scores are the original's own defaults
    // (`FUN_001d3d98`: the executable's tables, sorted as the game sorts).
    if fresh
        && let Some(records) = disc
            .iso
            .read("/SLUS_206.51")
            .ok()
            .and_then(|elf| original_save::default_records(&elf))
    {
        save.story.records = records;
    }
    // Player editor: roster.json's created-baller characters come from the save.
    for note in roster::with_save(&mut disc, &save.story)? {
        eprintln!("roster: {note}");
    }
    // A damaged or hand-edited save mustn't point past the disc's tables.
    let dropped = save
        .story
        .sanitize(disc.players.len(), disc.teams.len(), disc.moves.len());
    if dropped > 0 {
        eprintln!("save: dropped {dropped} entries that point past the disc's tables");
    }
    // Created ballers replace their create-a-player records everywhere.
    ballers::apply_all(&mut disc, &save.story);
    // Created teams join the database's teams.
    teams::apply_all(&mut disc, &save.story);
    // Online: the disc's own tables, and whether this roster may be hosted.
    let original = roster::Original::read(&mut disc)?;
    let host_check = roster::host_check(&mut disc, &original);
    if let Err(why) = &host_check.0 {
        eprintln!("roster: not for hosting online: {why}");
    }
    let mut match_setup = setup::MatchSetup::new(
        &disc,
        disc.find_team(&args.teams[0])?,
        disc.find_team(&args.teams[1])?,
    );
    match_setup.court = args.court;
    match_setup.rules = args.rules;
    // The saved options' difficulty (the original's default, Got Game, on
    // a fresh save) unless --difficulty chose one.
    if !args.difficulty_given {
        match_setup.rules.difficulty =
            Difficulty::from_level(save.story.settings.unwrap_or_default().difficulty);
    }
    match_setup.seed = args.seed.unwrap_or(2002);
    // The session's generator (`session.rs`): the clock's, as the boot
    // seeds it, unless the run must repeat: `--seed`, or a scripted run
    // (keys, a screenshot or an event log), which keeps 2002.
    let scripted = !args.keys.is_empty() || args.screenshot.is_some() || args.events.is_some();
    let mut generator = match args.seed {
        Some(seed) => session::Session::pinned(seed),
        None if scripted => session::Session::pinned(2002),
        None => session::Session::boot(),
    };
    // The loading cards' object, made at the boot with its draw.
    let loading_cards = loading::LoadingCards::new(generator.draw());
    match_setup.demo = args.demo;
    match_setup.watch = args.demo;
    // `--level2-at` plays the School's one game (its court and teams, the
    // person on the Lakers), where the original's Level 2 lesson is.
    if args.level2_at.is_some() {
        school::prepare(&disc, &mut match_setup)?;
    }
    let full_meters = args.full_meters;
    let level2_at = args.level2_at;
    let level2_script = args.level2_script;
    let play_by_play = args.play_by_play;
    // The match intros play when the boot movies would: not in scripted
    // or screenshot runs unless `--intro` asks.
    let match_intros = args
        .intro
        .unwrap_or(args.keys.is_empty() && args.screenshot.is_none());
    // Online: the creator loads its match first; a joiner waits in the
    // lobby for the server's.
    if let (Some(o), Err(why)) = (&args.online, &host_check.0)
        && (o.host.is_some() || o.create)
    {
        return Err(why.clone().into());
    }
    let online = args
        .online
        .as_ref()
        .map(|o| online::Online::new(o, Some(&original), clips.sim.clone()))
        .transpose()?;
    match_setup.online = online.is_some();
    let start = match &online {
        Some(o) if o.loads_first() => Screen::Loading,
        Some(_) => Screen::Lobby,
        None if args.quick => Screen::Loading,
        None if args
            .intro
            .unwrap_or(args.keys.is_empty() && args.screenshot.is_none()) =>
        {
            Screen::Boot
        }
        None => Screen::Menu,
    };

    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(app::window_plugin("Streetball Revival".to_string()))
            .set(render_plugin()),
        StreetRenderPlugin,
    ))
    .insert_resource(ClearColor(Color::srgb(0.53, 0.70, 0.86)))
    .insert_resource(GlobalAmbientLight {
        brightness: 400.0,
        ..default()
    })
    .insert_resource(Time::<Fixed>::from_hz(f64::from(TICK_HZ)))
    .insert_resource(disc)
    .insert_resource(original)
    .insert_resource(host_check)
    .insert_resource(setup::DiscPath(iso_path))
    .insert_resource(clips)
    .insert_resource(match_setup)
    // A game started from the menus waits on its loading screen for Cross
    // (the original's Continue); a quick match goes straight in.
    .insert_resource(loading_view::Waits(!args.quick))
    .insert_resource(generator)
    .insert_resource(loading_cards)
    .insert_resource(save)
    .insert_resource(names)
    .insert_resource(story::Running::default())
    // The boot (its loading screen, the card's check, the movies) plays
    // unless a match was asked for, or a scripted or screenshot run would
    // rather start at the title (`--intro` forces it).
    .insert_state(start)
    .add_plugins(credits::CreditsPlugin)
    .add_plugins((
        boot::BootPlugin,
        menu::MenuPlugin,
        attract::AttractPlugin,
        transition::TransitionPlugin,
        game::GamePlugin,
        audio::AudioPlugin,
        movie::MoviePlugin,
        sfx::SfxPlugin,
        ambience::AmbiencePlugin,
        ballfx::BallFxPlugin,
        trails::TrailsPlugin,
        squeak::SqueakPlugin,
        online::OnlinePlugin,
        pads::PadsPlugin,
        options::OptionsPlugin,
    ))
    .add_systems(
        Startup,
        (
            app::spawn_sun,
            art::start,
            look::start,
            bracket_view::start,
            cab_view::start,
            portraits::start,
        ),
    );
    if args.speed != 1.0 {
        let speed = args.speed;
        app.add_systems(Startup, move |mut time: ResMut<Time<Virtual>>| {
            time.set_relative_speed(speed);
            // Let the fixed-rate simulation catch up at high speeds.
            time.set_max_delta(std::time::Duration::from_secs_f32(0.25 * speed.max(1.0)));
        });
    }
    if let Some(o) = online {
        app.insert_resource(o);
    }
    if let Some(level2) = full_meters {
        app.insert_resource(setup::FullMeters { level2 });
    }
    if let Some(at) = level2_at {
        app.insert_resource(setup::Level2At::new(at, level2_script));
    }
    if play_by_play {
        app.insert_resource(hud::PlayByPlay);
    }
    if let Some(picture) = args.widescreen {
        app.insert_resource(options::PictureOverride(picture));
    }
    app.insert_resource(intro::Enabled(match_intros));
    if let Some(seconds) = args.demo_delay {
        app.insert_resource(attract::DelayOverride(seconds));
    }
    if let Some(path) = &args.events {
        app.insert_resource(
            game::EventLog::create(path)
                .map_err(|e| format!("--events {}: {e}", path.display()))?,
        );
    }
    if !args.keys.is_empty() {
        app.insert_resource(ScriptedKeys(args.keys, args.keys_real))
            .add_systems(
                PreUpdate,
                replay_keys
                    .after(bevy::input::InputSystems)
                    .before(transition::queue_presses),
            );
    }
    if let Some(path) = args.screenshot {
        // --at is game time. A match run counts the match's own clock, so
        // the capture lands on the same update every run; scripted menu
        // runs count real time.
        let scripted = app.world().contains_resource::<ScriptedKeys>();
        if let Some(every) = args.every {
            app.insert_resource(app::ScreenshotSeries {
                every,
                count: args.count.max(1),
            });
        }
        app.add_plugins(ScreenshotPlugin {
            path,
            delay_secs: if scripted {
                args.at / args.speed.max(0.01)
            } else {
                args.at
            },
        });
        if !scripted && !args.at_real {
            // The clock is read in the frame it is set, after the sim's
            // fixed updates, so `--at` lands on the first frame whose
            // match clock reaches it (update `--at` x 60 just run).
            app.init_resource::<app::ScreenshotClock>()
                .add_systems(Update, match_clock.before(app::ScreenshotSystems));
        } else if let Some(n) = args.at_transition {
            // Real seconds from the Nth transition's start.
            app.init_resource::<app::ScreenshotClock>().add_systems(
                Update,
                (move |t: Res<transition::Transition>,
                       time: Res<Time<Real>>,
                       mut clock: ResMut<app::ScreenshotClock>| {
                    clock.0 = n
                        .checked_sub(1)
                        .and_then(|k| t.starts.get(k))
                        .map(|&at| (time.elapsed_secs_f64() - at) as f32);
                })
                .after(transition::TransitionSystems)
                .before(app::ScreenshotSystems),
            );
        } else if !args.at_real {
            // The keys' clock, stopped through the screens' transitions.
            app.init_resource::<app::ScreenshotClock>().add_systems(
                Update,
                (|t: Res<transition::Transition>, mut clock: ResMut<app::ScreenshotClock>| {
                    clock.0 = Some(t.clock as f32);
                })
                .after(transition::TransitionSystems)
                .before(app::ScreenshotSystems),
            );
        }
    }
    app.run();
    Ok(())
}

/// The renderer compiles each pipeline when it is first needed instead of
/// in the background: otherwise a match's first frames draw no 3D at all (a
/// black court for a second or two after the loading picture) until every
/// pipeline it uses is ready. The compile now happens behind the loading
/// picture's last frame.
fn render_plugin() -> bevy::render::RenderPlugin {
    bevy::render::RenderPlugin {
        synchronous_pipeline_compilation: true,
        ..default()
    }
}

/// `--keys`: a comma-separated list of `seconds:key`, e.g. `1:enter,1.5:down`,
/// each a tap, or `seconds:key*hold` to hold it that many seconds.
/// Keys: up, down, left, right, enter, esc, space, or a letter.
fn parse_keys(list: &str) -> Result<Vec<(f32, KeyCode, f32)>, String> {
    list.split(',')
        .map(|item| {
            let (at, key) = item
                .split_once(':')
                .ok_or(format!("--keys: {item:?} is not seconds:key"))?;
            let at: f32 = at.trim().parse().map_err(|e| format!("--keys: {e}"))?;
            let (key, hold) = match key.split_once('*') {
                Some((key, hold)) => (
                    key,
                    hold.trim().parse().map_err(|e| format!("--keys: {e}"))?,
                ),
                None => (key, 0.0),
            };
            let key = match key.trim().to_ascii_lowercase().as_str() {
                "up" => KeyCode::ArrowUp,
                "down" => KeyCode::ArrowDown,
                "left" => KeyCode::ArrowLeft,
                "right" => KeyCode::ArrowRight,
                "enter" => KeyCode::Enter,
                "esc" => KeyCode::Escape,
                "space" => KeyCode::Space,
                "shift" => KeyCode::ShiftLeft,
                letter if letter.len() == 1 && letter.as_bytes()[0].is_ascii_lowercase() => {
                    const LETTERS: [KeyCode; 26] = [
                        KeyCode::KeyA,
                        KeyCode::KeyB,
                        KeyCode::KeyC,
                        KeyCode::KeyD,
                        KeyCode::KeyE,
                        KeyCode::KeyF,
                        KeyCode::KeyG,
                        KeyCode::KeyH,
                        KeyCode::KeyI,
                        KeyCode::KeyJ,
                        KeyCode::KeyK,
                        KeyCode::KeyL,
                        KeyCode::KeyM,
                        KeyCode::KeyN,
                        KeyCode::KeyO,
                        KeyCode::KeyP,
                        KeyCode::KeyQ,
                        KeyCode::KeyR,
                        KeyCode::KeyS,
                        KeyCode::KeyT,
                        KeyCode::KeyU,
                        KeyCode::KeyV,
                        KeyCode::KeyW,
                        KeyCode::KeyX,
                        KeyCode::KeyY,
                        KeyCode::KeyZ,
                    ];
                    LETTERS[usize::from(letter.as_bytes()[0] - b'a')]
                }
                other => return Err(format!("--keys: unknown key {other:?}")),
            };
            Ok((at, key, hold))
        })
        .collect()
}

/// The match's own clock for a screenshot run: its updates so far, in
/// seconds, once a match is running.
fn match_clock(sim: Option<Res<game::Sim>>, mut clock: ResMut<app::ScreenshotClock>) {
    clock.0 = sim.map(|sim| sim.world.tick as f32 / TICK_HZ as f32);
}

/// Key presses still to replay, in time order.
/// The scripted presses, and whether their times are real seconds.
#[derive(Resource)]
struct ScriptedKeys(Vec<(f32, KeyCode, f32)>, bool);

fn replay_keys(
    clock: Res<transition::Transition>,
    time: Res<Time<Real>>,
    mut script: ResMut<ScriptedKeys>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut held: Local<Vec<(KeyCode, f32)>>,
    mut typed: MessageWriter<bevy::input::keyboard::KeyboardInput>,
    window: Query<Entity, With<bevy::window::PrimaryWindow>>,
) {
    // The script's own clock stops while a screen's transition runs, so a
    // press never meets one (the original would drop it or hold it for the
    // next screen) and scripts reach the pages they did before.
    let now = if script.1 {
        time.elapsed_secs()
    } else {
        clock.clock as f32
    };
    // A tap is up again the next frame, a hold once its time is out.
    held.retain(|&(key, until)| {
        let up = until <= now;
        if up {
            keys.release(key);
        }
        !up
    });
    while let Some(&(at, key, hold)) = script.0.first() {
        if at > now {
            break;
        }
        keys.press(key);
        held.push((key, at + hold));
        script.0.remove(0);
        // Typing reads the key messages: letters, Space, Enter and Esc go
        // there too, so scripted runs can type a name.
        if let Ok(window) = window.single() {
            use bevy::input::{ButtonState, keyboard::Key};
            let letter = format!("{key:?}")
                .strip_prefix("Key")
                .filter(|l| l.len() == 1)
                .map(str::to_ascii_lowercase);
            let logical = match (key, letter) {
                (KeyCode::Enter, _) => Some(Key::Enter),
                (KeyCode::Escape, _) => Some(Key::Escape),
                (KeyCode::Space, _) => Some(Key::Space),
                (_, Some(l)) => Some(Key::Character(l.as_str().into())),
                _ => None,
            };
            if let Some(logical_key) = logical {
                typed.write(bevy::input::keyboard::KeyboardInput {
                    key_code: key,
                    logical_key,
                    state: ButtonState::Pressed,
                    text: None,
                    repeat: false,
                    window,
                });
            }
        }
    }
}

#[cfg(test)]
mod render_tests {
    #[test]
    fn pipelines_compile_before_a_frame_needs_them() {
        // The black first second of a match was the renderer skipping every
        // draw whose pipeline was still compiling in the background.
        assert!(super::render_plugin().synchronous_pipeline_compilation);
    }
}
