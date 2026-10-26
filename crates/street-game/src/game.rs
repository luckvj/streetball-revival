//! A match: loading it, spawning it, stepping the simulation, pausing, and
//! the results at the end.

use bevy::{mesh::skinning::SkinnedMeshInverseBindposes, prelude::*};
use street_render::animation::Animator;
use street_sim::{Basket, Goal, Input, PassStyle, Phase, World as SimWorld};

use crate::{
    Screen, clip, controls,
    menu::{self, MenuInput},
    school,
    setup::{self, Disc, GameClips, PlayerNames, Scene, TeamNames},
    story, view,
};

/// The simulation and the controls for its next tick.
#[derive(Resource)]
pub struct Sim {
    pub world: SimWorld,
    pub inputs: Vec<Input>,
    /// The latest notable play, for the HUD.
    pub call: String,
    /// How each player last passed, to pick the throwing animation.
    pub pass_style: Vec<PassStyle>,
}

/// Box score, per player.
#[derive(Resource, Default)]
pub struct Stats {
    pub points: Vec<u32>,
    pub dunks: Vec<u32>,
    pub steals: Vec<u32>,
    pub blocks: Vec<u32>,
    pub fakeouts: Vec<u32>,
    /// Level 1 and level 2 gamebreaker baskets, alley-oops finished,
    /// long range shots made and pass outs thrown, for the High Scores.
    pub gb1: Vec<u32>,
    pub gb2: Vec<u32>,
    pub oops: Vec<u32>,
    pub long: Vec<u32>,
    pub passouts: Vec<u32>,
    /// Rebounds and each player's trick points, for the post-game screens.
    pub rebounds: Vec<u32>,
    pub tricks: Vec<u32>,
}

impl Stats {
    pub fn new(players: usize) -> Self {
        Self {
            points: vec![0; players],
            dunks: vec![0; players],
            steals: vec![0; players],
            blocks: vec![0; players],
            fakeouts: vec![0; players],
            gb1: vec![0; players],
            gb2: vec![0; players],
            oops: vec![0; players],
            long: vec![0; players],
            passouts: vec![0; players],
            rebounds: vec![0; players],
            tricks: vec![0; players],
        }
    }

    /// The counts the original's stats object keeps, from the sim's
    /// ([`street_sim::stats`]): points (stat 0), dunks (1), alley-oops
    /// (2), rebounds (4), steals (5, trick counters and interceptions
    /// included), blocks (6), threes made (8), trick points (11) and the
    /// Level 1 and 2 gamebreaker baskets (12, 13). The rest (fakeouts,
    /// pass outs, Level 2s started) are the rewrite's own tallies.
    pub fn sync(&mut self, s: &street_sim::stats::Stats) {
        use street_sim::stats::*;
        for (i, p) in s.players.iter().enumerate() {
            let rows: [(&mut Vec<u32>, usize); 9] = [
                (&mut self.points, POINTS),
                (&mut self.dunks, DUNKS),
                (&mut self.oops, ALLEY_OOPS),
                (&mut self.rebounds, REBOUNDS),
                (&mut self.steals, STEALS),
                (&mut self.blocks, BLOCKS),
                (&mut self.long, THREES),
                (&mut self.tricks, TRICK_POINTS),
                (&mut self.gb1, LEVEL1),
            ];
            for (row, stat) in rows {
                if let Some(v) = row.get_mut(i) {
                    *v = p[stat].max(0) as u32;
                }
            }
            if let Some(v) = self.gb2.get_mut(i) {
                *v = p[LEVEL2].max(0) as u32;
            }
        }
    }
}

/// Whether the match is paused, and the pause screen's state
/// ([`crate::pause`]). Online the menu opens (`open`) but the game plays
/// on (`on` stays false).
#[derive(Resource, Default)]
pub struct Paused {
    pub on: bool,
    open: bool,
    /// The pause screen's row ([`crate::pause::row`]).
    cursor: usize,
    /// Why the game holds itself paused (a controller unplugged): shown
    /// in a dialog, and only Quit works until it is released.
    notice: Option<&'static str>,
    /// Quit Game pressed: "Are you sure you want to quit?" is up.
    asking: bool,
    /// "You're being challenged!" is up (`challengercomes.txt`, screen
    /// class 100).
    challenge: bool,
    /// The dialog's lit row.
    choice: usize,
    page: crate::pause::Page,
    /// The controller panel's cursor (1 layout, 2 view, 3 vibration) and
    /// the song list's.
    panel_cursor: usize,
    trax_cursor: usize,
    /// The controller panel's view (offence, defence, ...).
    panel_view: u8,
    /// Updates since the screen opened: the lit row pulses by them.
    ticks: u64,
}

impl Paused {
    /// Whether the pause screen (or one of its dialogs) is up: the HUD
    /// hides under it (`FUN_002b38a0`).
    pub fn open(&self) -> bool {
        self.open
    }

    /// Holds the match paused with `why` on the pause screen.
    pub fn hold(&mut self, why: &'static str) {
        if self.notice != Some(why) {
            self.notice = Some(why);
            self.open = true;
            self.choice = 0;
        }
    }

    /// Lets go of a hold made with `why`.
    pub fn release(&mut self, why: &'static str) {
        if self.notice == Some(why) {
            self.notice = None;
            self.open = false;
        }
    }
}

/// Seconds since the final buzzer, before the results show.
#[derive(Resource, Default)]
struct Finished(f32);

/// A replay on screen (the replay manager's state 2, [`crate::tape`]), for
/// the systems that keep out of replays. Set by [`crate::tape::step`].
#[derive(Resource, Default)]
pub struct Replay {
    pub playing: Option<Playing>,
}

/// The replay on screen: its mode (`+0x24`, [`crate::tape::mode`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Playing {
    pub mode: u32,
}

/// The simulation's events from the ticks stepped in the last fixed update,
/// for sound and effects. `replay` is set for an update the replay manager
/// held the game to show a recorded frame (no events then). (Read this
/// rather than `Sim.world.events`: a replay puts a recorded world on
/// screen.)
#[derive(Resource, Default)]
pub struct TickEvents {
    pub events: Vec<street_sim::Event>,
    pub replay: bool,
}

/// The lesson that was just passed (`Ok`) or failed on a native callback
/// (`Err`), for the results screen; neither for a plain match.
#[derive(Resource, Default)]
pub struct LessonPassed(pub Option<Result<usize, ()>>);

/// `--events`: the match's events, the ball's state changes and the
/// players' action changes, one line each with the update they happened
/// in (`update / 60` is `--at`'s game time), written as they happen so a
/// screenshot can be timed by the sim's clock.
#[derive(Resource)]
pub struct EventLog {
    file: std::io::BufWriter<std::fs::File>,
    ball: Option<street_sim::BallState>,
    actions: Vec<String>,
}

impl EventLog {
    pub fn create(path: &std::path::Path) -> std::io::Result<Self> {
        Ok(Self {
            file: std::io::BufWriter::new(std::fs::File::create(path)?),
            ball: None,
            actions: Vec::new(),
        })
    }

    /// After an update of `world`: its events and what changed.
    fn note(&mut self, world: &SimWorld, events: &[street_sim::Event]) {
        use std::io::Write;
        // `World::tick` counts completed updates; the one just run is
        // one less.
        let update = world.tick.saturating_sub(1);
        if self.ball != Some(world.ball.state) {
            self.ball = Some(world.ball.state);
            let _ = writeln!(self.file, "{update}\tball {:?}", world.ball.state);
        }
        self.actions.resize(world.players.len(), String::new());
        for (i, p) in world.players.iter().enumerate() {
            // The action's variant and identity, not its running counters.
            let kind = match p.action {
                street_sim::Action::None => "None".to_string(),
                street_sim::Action::Shooting { script, fade, .. } => {
                    format!("Shooting script {script} fade {fade}")
                }
                street_sim::Action::Dunking { row, style, .. } => {
                    format!("Dunking row {row} {style:?}")
                }
                street_sim::Action::Layup { row, .. } => format!("Layup row {row}"),
                street_sim::Action::PumpFake { script, .. } => format!("PumpFake script {script}"),
                street_sim::Action::Passing { row, .. } => format!("Passing row {row}"),
                other => format!("{other:?}")
                    .split([' ', '{'])
                    .next()
                    .unwrap_or_default()
                    .to_string(),
            };
            if self.actions[i] != kind {
                self.actions[i] = kind;
                let _ = writeln!(self.file, "{update}\tplayer {i} {}", self.actions[i]);
            }
        }
        for event in events {
            let _ = writeln!(self.file, "{update}\t{event:?}");
        }
        let _ = self.file.flush();
    }

    /// A line of the audio's own (the commentator's lines starting and
    /// ending), in the update `world` just ran.
    pub(crate) fn audio(&mut self, world: &SimWorld, text: &str) {
        use std::io::Write;
        let update = world.tick.saturating_sub(1);
        let _ = writeln!(self.file, "{update}\t{text}");
        let _ = self.file.flush();
    }
}

/// The systems that step the world: [`step_sim`] offline,
/// [`crate::online::net_step`] online. Whatever reads [`TickEvents`] runs
/// after this.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct SimStep;

#[derive(Component)]
struct ResultsText;

pub struct GamePlugin;

impl Plugin for GamePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(crate::boards::BoardsPlugin)
            .init_resource::<Paused>()
            .init_resource::<Finished>()
            .init_resource::<Replay>()
            .init_resource::<crate::tape::Tape>()
            .init_resource::<crate::tape::GamebreakerTask>()
            .init_resource::<crate::tape::Director>()
            .init_resource::<crate::referee::Referee>()
            .init_resource::<view::ActiveCamera>()
            .init_resource::<LessonPassed>()
            .init_resource::<school::LessonRun>()
            .init_resource::<crate::school_menu::LessonMenu>()
            .init_resource::<crate::school_prompt::Prompt>()
            .init_resource::<crate::school_prompt::Page>()
            .init_resource::<TickEvents>()
            .init_resource::<crate::intro::Variety>()
            .init_resource::<crate::loading::LoadingCards>()
            .init_resource::<crate::loading::Mode>()
            .init_resource::<crate::loading_view::LoadingScreen>()
            .init_resource::<crate::loading_view::Waits>()
            .init_resource::<crate::hud::HudState>()
            .init_resource::<AfterGame>()
            .init_resource::<Rematch>()
            .init_resource::<crate::rematch_rules::RematchRules>()
            .add_systems(Startup, crate::hud::start)
            .add_systems(OnEnter(Screen::Loading), spawn_loading)
            .add_systems(
                Update,
                (load_after_a_frame, loading_keys, draw_loading)
                    .chain()
                    .run_if(in_state(Screen::Loading)),
            )
            .add_systems(OnEnter(Screen::Returning), spawn_return)
            .add_systems(
                Update,
                (
                    return_after_the_load.run_if(crate::transition::idle),
                    leave_return,
                    draw_loading.run_if(crate::transition::draws),
                )
                    .chain()
                    .before(crate::transition::TransitionSystems)
                    .run_if(in_state(Screen::Returning)),
            )
            .add_systems(
                OnEnter(Screen::Playing),
                (
                    spawn_scene,
                    view::hide_near_side,
                    view::spawn_hud,
                    spawn_overlays,
                    open_school_menu,
                    crate::intro::start,
                    |mut hud: ResMut<crate::hud::HudState>| *hud = default(),
                    |mut fx: ResMut<crate::screen_fx::ScreenFx>| *fx = default(),
                    // The game's start: not finished, its options copied.
                    |mut after: ResMut<AfterGame>, opts: Res<crate::options::Options>| {
                        *after = AfterGame {
                            options: Some(opts.0),
                            ..default()
                        };
                    },
                )
                    .chain(),
            )
            .add_systems(OnExit(Screen::Playing), leave)
            // A replay's recorded joints over the animator's pose.
            .add_systems(
                PostUpdate,
                crate::tape::pose_bones
                    .after(street_render::animation::pose_skeletons)
                    .before(TransformSystems::Propagate),
            )
            // The heads turned to their looks, over the clips' pose.
            .add_systems(
                PostUpdate,
                crate::head_turn::turn_heads
                    .after(crate::tape::pose_bones)
                    .before(TransformSystems::Propagate)
                    .run_if(in_state(Screen::Playing)),
            )
            .add_systems(
                FixedUpdate,
                (
                    controls::fill_inputs.run_if(not(crate::online::is_online)),
                    (
                        step_sim.run_if(not(crate::online::is_online)),
                        crate::online::net_step.run_if(crate::online::is_online),
                    )
                        .in_set(SimStep),
                    crowd_reactions,
                    // The replay director, manager and gamebreaker task, in
                    // the original's task order (crate::tape).
                    crate::tape::step,
                    crate::referee::step,
                )
                    .chain()
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on)
                    .run_if(crate::intro::finished),
            )
            .add_systems(
                FixedUpdate,
                camera_step(view::step_camera.after(crate::referee::step)),
            )
            .add_systems(
                FixedUpdate,
                show_rims
                    .after(SimStep)
                    .before(street_render::net::step)
                    .run_if(in_state(Screen::Playing)),
            )
            // The HUD's update (the Overlays task after the game's), through
            // the intro and replays too.
            .add_systems(
                FixedUpdate,
                (crate::hud::step, school_prompt_step)
                    .after(crate::referee::step)
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on),
            )
            // The screen's layers and blur (the gamebreaker overlay's and a
            // staged script's), after the replays, through them too.
            .init_resource::<crate::screen_fx::ScreenFx>()
            .add_systems(
                FixedUpdate,
                crate::screen_fx::step
                    .after(crate::tape::step)
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on),
            )
            // The copy into the shown buffer on every screen's last camera.
            .add_systems(Update, crate::screen_fx::show)
            // The crowd runs every tick of a match on screen (its intro and
            // replays too), held only by the pause menu (`FUN_0030c9e0`
            // skips it while `DAT_00405388` is set).
            .add_systems(
                FixedUpdate,
                street_render::crowd::step
                    .after(crowd_reactions)
                    .run_if(in_state(Screen::Playing))
                    .run_if(|p: Res<Paused>| !p.on),
            )
            .add_systems(
                Update,
                (
                    controls::read,
                    crate::intro::step,
                    (
                        view::sync_players,
                        view::sync_hands,
                        crate::referee::sync,
                        view::sync_ball,
                        view::sync_indicator,
                        view::follow_camera,
                        view::cull_scenery,
                    )
                        .chain(),
                    view::update_hud,
                    crate::hud::draw,
                    school_menu_keys.before(pause),
                    school_page_keys.before(pause),
                    pause,
                    draw_pause.after(pause),
                    draw_school_menu.after(pause),
                    results,
                )
                    .run_if(in_state(Screen::Playing)),
            );
    }
}

/// The camera's 60 Hz step runs whenever a match is on screen and not
/// paused: through the intro and a gamebreaker's replay too, whose authored
/// tracks it follows, not only once play starts (G02).
fn camera_step<M>(
    system: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
) -> bevy::ecs::schedule::ScheduleConfigs<bevy::ecs::system::ScheduleSystem> {
    system
        .run_if(in_state(Screen::Playing))
        .run_if(|p: Res<Paused>| !p.on)
}

#[allow(clippy::too_many_arguments)]
fn spawn_loading(
    mut commands: Commands,
    setup: Res<setup::MatchSetup>,
    mut disc: ResMut<Disc>,
    mut images: ResMut<Assets<Image>>,
    mut cards: ResMut<crate::loading::LoadingCards>,
    running: Res<story::Running>,
    mut screen: ResMut<crate::loading_view::LoadingScreen>,
    (time, look, opts, mut mode, rematch): (
        Res<Time<Real>>,
        Option<Res<crate::look::Look>>,
        Res<crate::options::Options>,
        ResMut<crate::loading::Mode>,
        Res<Rematch>,
    ),
) {
    if rematch.0 {
        // No load to show: the frame stays black while the rewrite
        // rebuilds the match.
        commands.spawn((
            Camera2d,
            Camera {
                clear_color: ClearColorConfig::Custom(Color::BLACK),
                ..default()
            },
            DespawnOnExit(Screen::Loading),
        ));
        *screen = crate::loading_view::LoadingScreen {
            since: time.elapsed_secs_f64(),
            ..default()
        };
        return;
    }
    commands.spawn((Camera2d, DespawnOnExit(Screen::Loading)));
    // The game's own loading picture (`crate::loading_view` draws it and
    // the screen's widgets), by the front end's mode as `FUN_0029c3a8`
    // reads it: the attract's game (the rewrite's `--demo` run), a story's
    // or the School's, Pick Up. The load back reads the same mode.
    *mode = match running.0 {
        Some(story::Run::School(_)) => crate::loading::Mode::School,
        Some(story::Run::League(..)) => crate::loading::Mode::League,
        Some(story::Run::StoryEvent(_)) => crate::loading::Mode::Legend,
        None if setup.watch => crate::loading::Mode::Attract,
        None => crate::loading::Mode::PickUp,
    };
    let (k, tip) = cards.next(mode.kind());
    let (mode_line, difficulty_line) = look.as_deref().map_or_else(Default::default, |look| {
        (
            crate::loading_view::mode_line(look, running.0, menu::mode_of(&setup.rules)),
            match running.0 {
                // `FUN_0029cbc0` mode 1: "Instructor : Stretch Monroe".
                Some(story::Run::School(_)) => look
                    .text("setchallenge.txt", 56)
                    .unwrap_or("Instructor : Stretch Monroe")
                    .to_owned(),
                _ => crate::loading_view::difficulty_line(look, setup.rules.difficulty),
            },
        )
    });
    // The first player's controller layout (`FUN_002148c8(.., game +0xd4)`).
    *screen = crate::loading_view::LoadingScreen {
        picture: crate::loading::picture(&mut disc, &mut images, k),
        tip,
        street_tip: None,
        stage: crate::loading_view::Stage::Loading,
        since: time.elapsed_secs_f64(),
        mode_line,
        difficulty_line,
        layout: opts.0.layouts[0],
        school: matches!(running.0, Some(story::Run::School(_))),
        demo: *mode == crate::loading::Mode::Attract,
    };
}

/// The loading screen's pieces, each update (going into a game and on the
/// load back).
fn draw_loading(
    mut commands: Commands,
    state: Res<State<Screen>>,
    screen: Res<crate::loading_view::LoadingScreen>,
    time: Res<Time<Real>>,
    look: Option<Res<crate::look::Look>>,
    art: Option<Res<crate::art::Art>>,
    mut labels: ResMut<crate::look::Labels>,
    mut images: ResMut<Assets<Image>>,
    mut slots: Query<(
        &crate::look::Slot,
        &mut ImageNode,
        &mut Node,
        &mut Visibility,
    )>,
    (card, gm, mut panels, rematch): (
        Res<crate::card::Card>,
        Option<Res<crate::boards::Gm>>,
        Local<crate::boards::Panels>,
        Res<Rematch>,
    ),
) {
    let Some(look) = look.as_deref() else {
        return;
    };
    if rematch.0 && *state.get() == Screen::Loading {
        crate::look::show(&mut commands, &mut slots, &[], *state.get());
        return;
    }
    let ms = (time.elapsed_secs_f64() - screen.since) * 1000.0;
    let backdrop = art.as_deref().map(|a| {
        a.backdrop(if screen.stage == crate::loading_view::Stage::Welcome {
            crate::loading_view::WELCOME_BACKDROP
        } else {
            crate::loading_view::CONTROLS_BACKDROP
        })
    });
    let pieces = crate::loading_view::pieces(
        look,
        &mut labels,
        &mut images,
        art.as_deref().and_then(|a| a.loading.as_ref()),
        backdrop,
        &screen,
        ms,
    );
    let mut pieces = pieces;
    if let (Some(flow), Some(gm)) = (card.flow.as_ref(), gm.as_deref()) {
        pieces.extend(crate::card::pieces(
            look,
            gm,
            &mut labels,
            &mut images,
            &mut panels,
            flow,
            crate::card::DIM,
            (ms * 0.06) as u64,
        ));
    }
    crate::look::show(&mut commands, &mut slots, &pieces, *state.get());
}

/// Cross on the loaded screen (`FUN_0029b120`, `FUN_0029c578`): with Help
/// on, the controller pages (Offense, Defense), then the game.
fn loading_keys(
    keys: Res<ButtonInput<KeyCode>>,
    pads: Query<&Gamepad>,
    mut screen: ResMut<crate::loading_view::LoadingScreen>,
    opts: Res<crate::options::Options>,
    mut next: ResMut<NextState<Screen>>,
) {
    use crate::loading_view::Stage;
    if matches!(screen.stage, Stage::Loading) {
        return;
    }
    let cross = keys.any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ])
        || pads.iter().any(|g| g.just_pressed(GamepadButton::South));
    if !cross {
        return;
    }
    screen.stage = match screen.stage {
        Stage::Continue if opts.0.help => Stage::Controls(0),
        Stage::Controls(0) => Stage::Controls(1),
        // Street School's welcome page last (`FUN_0029ca10`).
        Stage::Continue | Stage::Controls(_) if screen.school => Stage::Welcome,
        _ => {
            next.set(Screen::Playing);
            return;
        }
    };
}

/// How long the loading picture shows at least.
const LOADING_SECONDS: f64 = 1.2;

/// A Pick Up rematch is under way: the original builds the new game in
/// the game's own task (the pause's Quit Game REMATCH, `FUN_002bd3f8`, and
/// the post-game dialog's, both opening the rematch rules `FUN_0025dc88`
/// over the court) and shows no load, so the rewrite rebuilds its match
/// without the loading screen's picture, wait or Continue.
#[derive(Resource, Default)]
pub struct Rematch(pub bool);

/// What the load back's save to the card needs from the game just played
/// (`DAT_00400054`): whether it was finished (bit 2, `FUN_002f3918`: the
/// game's phase 9) and the options as it began (`FUN_002f2d58`'s copy,
/// which `FUN_002f2e50` compares at its end for bit 4); and, once the load
/// back has decided, whether it saves.
#[derive(Resource, Default)]
pub struct AfterGame {
    pub finished: bool,
    pub options: Option<street_sim::records::Settings>,
    pub saves: bool,
}

/// How long the load back's picture stands: the original's load of the
/// front end (`FUN_0029b340`'s `+0xf4`), 327 to 329 hundredths in each of
/// the eleven loads measured in the runtime (after Pick Up, Be A Legend,
/// the School's and the attract's games), the shortest of them.
pub const RETURN_SECONDS: f64 = 3.27;

/// The load back to the front end after a game (class 0x2f, the Front End
/// task's restart `FUN_002e1bf0`, `FUN_0029d368`): its picture by the
/// game's mode (`FUN_0029d420`, [`crate::loading::LoadingCards::back`]) with
/// the spinner, the ring and "Loading" (`FUN_0029d4d8`: `FUN_0029ac18`;
/// Pick Up's Street Tips page `FUN_0029b720`, which also draws once from the
/// shared generator for nothing it keeps).
#[allow(clippy::too_many_arguments)]
fn spawn_return(
    mut commands: Commands,
    mut disc: ResMut<Disc>,
    mut images: ResMut<Assets<Image>>,
    mut cards: ResMut<crate::loading::LoadingCards>,
    mode: Res<crate::loading::Mode>,
    save: Res<story::Save>,
    mut screen: ResMut<crate::loading_view::LoadingScreen>,
    (time, art, session): (
        Res<Time<Real>>,
        Option<Res<crate::art::Art>>,
        Option<ResMut<crate::session::Session>>,
    ),
    (mut after, menus, opts, setup): (
        ResMut<AfterGame>,
        Res<menu::Menus>,
        Res<crate::options::Options>,
        Res<setup::MatchSetup>,
    ),
) {
    commands.spawn((Camera2d, DespawnOnExit(Screen::Returning)));
    // The card's save after the load (`FUN_0029d5a8`); the rewrite's own
    // games (online, a watched one) have none.
    let changed = after
        .options
        .is_some_and(|o| crate::options::changed(&o, &opts.0));
    after.saves = !setup.online
        && crate::card::saves_after_game(
            *mode,
            after.finished,
            changed,
            menus.next_saves_itself(&save.story),
        );
    let owned = crate::loading::owned_street_tips(&disc.cards, &save.story);
    let career = crate::loading::Career::of(&save.story);
    let back = cards.back(*mode, owned, career);
    let (picture, tip, street_tip) = match back {
        crate::loading::Back::StreetTip(card) => {
            if let Some(mut session) = session {
                session.draw();
            }
            let backdrop = art.map(|a| a.backdrop(crate::loading::STREET_TIPS_BACKDROP));
            (backdrop, None, Some(card))
        }
        crate::loading::Back::Tip(tip) => (None, Some(tip), None),
        crate::loading::Back::Picture(_) => (None, None, None),
    };
    let picture = picture.or_else(|| {
        back.picture()
            .and_then(|k| crate::loading::picture(&mut disc, &mut images, k))
    });
    *screen = crate::loading_view::LoadingScreen {
        picture,
        tip,
        street_tip,
        stage: crate::loading_view::Stage::Loading,
        since: time.elapsed_secs_f64(),
        ..default()
    };
}

/// The load back takes no press (its load runs in one call, `FUN_0029b340`)
/// and at the load's end goes straight on (`FUN_0029d538` hides Continue,
/// `FUN_0029d5a8` opens the next screen): the menus after
/// [`RETURN_SECONDS`], or first the card's save over the dimmed screen
/// (`FUN_002d8460(.., 0x3f)`, [`AfterGame::saves`]).
fn return_after_the_load(
    screen: Res<crate::loading_view::LoadingScreen>,
    time: Res<Time<Real>>,
    mut next: ResMut<NextState<Screen>>,
    (mut card, mut after, mut save, opts, input): (
        ResMut<crate::card::Card>,
        ResMut<AfterGame>,
        ResMut<story::Save>,
        Res<crate::options::Options>,
        MenuInput,
    ),
) {
    if time.elapsed_secs_f64() - screen.since
        < crate::pace::hold(RETURN_SECONDS, crate::pace::PICTURE)
    {
        return;
    }
    if std::mem::take(&mut after.saves) {
        let holds = crate::card::Holds::of(&save.path);
        card.flow = Some(crate::card::Flow::new(
            crate::card::Op::Autosave,
            holds,
            opts.0.autosave,
        ));
    }
    if let Some(flow) = card.flow.as_mut() {
        crate::card::keys(flow, &input, f64::from(time.delta_secs()));
        if let Some(crate::card::Work::Write) = flow.work() {
            crate::card::write(flow, &mut save);
        }
        if !flow.done {
            return;
        }
        card.flow = None;
    }
    next.set(Screen::Menu);
}

/// The load back hands over to the front end's next screen through that
/// screen's transition (`0x2f`'s calls: forward to Update Team and the
/// completion boards, back to the rest), the menus' state once its movie
/// has played.
fn leave_return(
    mut next: ResMut<NextState<Screen>>,
    mut t: ResMut<crate::transition::Transition>,
    menus: Res<menu::Menus>,
    save: Res<story::Save>,
    time: Res<Time<Real>>,
) {
    if !matches!(*next, NextState::Pending(Screen::Menu)) || !t.idle() {
        return;
    }
    let Some((to, kind)) = menu::arrival(&menus, &save.story) else {
        return;
    };
    next.reset();
    t.begin(
        crate::transition::Change::new(0x2f, to, kind, false, Some(Screen::Menu)),
        time.elapsed_secs_f64(),
    );
}

/// Load once the loading screen has been drawn and shown a moment.
fn load_after_a_frame(world: &mut World, mut frames: Local<u32>, mut since: Local<Option<f64>>) {
    // Loaded: the screen waits for Cross (`loading_keys`).
    if world.resource::<crate::loading_view::LoadingScreen>().stage
        != crate::loading_view::Stage::Loading
    {
        return;
    }
    *frames += 1;
    // The loading picture stays up a moment (the PS2's loads take seconds);
    // a rematch has none to show.
    let rematch = world.resource::<Rematch>().0;
    let now = world.resource::<Time<Real>>().elapsed_secs_f64();
    let start = *since.get_or_insert(now);
    if !rematch
        && (*frames < 3
            || now - start < crate::pace::hold(LOADING_SECONDS, crate::pace::LOAD_FLOOR))
    {
        return;
    }
    *frames = 0;
    *since = None;
    match setup::load_match(world) {
        Ok(()) => {
            if let Some(story::Run::School(lesson)) = world.resource::<story::Running>().0 {
                let selected = world
                    .resource::<controls::Control>()
                    .first_player()
                    .unwrap_or(0);
                let seen = school::pages_seen(&world.resource::<crate::story::Save>().story);
                school::stage_world(
                    &mut world.resource_mut::<Sim>().world,
                    lesson,
                    selected,
                    seen,
                    &[],
                );
            }
            let next = if crate::online::mark_loaded(world) {
                Screen::Lobby
            } else {
                Screen::Playing
            };
            // A game from the menus waits for Cross (`FUN_0029b438`); the
            // attract, online and scripted games and a rematch go straight
            // in.
            let setup = world.resource::<setup::MatchSetup>();
            let waits = world.resource::<crate::loading_view::Waits>().0
                && !setup.demo
                && !setup.online
                && !rematch
                && next == Screen::Playing;
            world.resource_mut::<Rematch>().0 = false;
            if waits {
                world
                    .resource_mut::<crate::loading_view::LoadingScreen>()
                    .stage = crate::loading_view::Stage::Continue;
            } else {
                world.resource_mut::<NextState<Screen>>().set(next);
            }
        }
        Err(e) => {
            error!("could not load the match: {e}");
            world.resource_mut::<menu::Message>().0 = format!("Could not load the match: {e}");
            world.resource_mut::<NextState<Screen>>().set(Screen::Menu);
        }
    }
}

fn spawn_scene(
    mut commands: Commands,
    scene: Res<Scene>,
    disc: Res<Disc>,
    clips: Res<GameClips>,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut images: ResMut<Assets<Image>>,
    mut bindposes: ResMut<Assets<SkinnedMeshInverseBindposes>>,
    mut paused: ResMut<Paused>,
    mut finished: ResMut<Finished>,
    mut replay: ResMut<Replay>,
    (mut tape, mut gamebreaker_task, mut director, mut referee): (
        ResMut<crate::tape::Tape>,
        ResMut<crate::tape::GamebreakerTask>,
        ResMut<crate::tape::Director>,
        ResMut<crate::referee::Referee>,
    ),
    mut lesson_passed: ResMut<LessonPassed>,
    mut lesson_run: ResMut<school::LessonRun>,
    sim: Res<Sim>,
) {
    *paused = Paused::default();
    *finished = Finished::default();
    *replay = Replay::default();
    *tape = crate::tape::Tape::default();
    *gamebreaker_task = crate::tape::GamebreakerTask::default();
    *director = crate::tape::Director::default();
    *referee = crate::referee::Referee::default();
    lesson_passed.0 = None;
    *lesson_run = school::LessonRun::default();
    let scoped = DespawnOnExit(Screen::Playing);
    for root in street_render::court::spawn(
        &mut commands,
        &scene.court,
        &mut meshes,
        &mut materials,
        &mut images,
    ) {
        commands.entity(root).insert(scoped.clone());
    }
    for (i, assets) in scene.players.iter().enumerate() {
        let player = street_render::player::spawn(
            &mut commands,
            assets,
            Transform::default(),
            &mut meshes,
            &mut materials,
            &mut images,
            &mut bindposes,
        );
        commands.entity(player).insert((
            view::SimPlayer(i),
            Animator::new(clips.get(clip::STAND)),
            scoped.clone(),
        ));
    }
    // The referee, hidden until the intro's end (FUN_00142368).
    if let Some(assets) = &scene.referee {
        let referee = street_render::player::spawn(
            &mut commands,
            assets,
            Transform::default(),
            &mut meshes,
            &mut materials,
            &mut images,
            &mut bindposes,
        );
        commands.entity(referee).insert((
            crate::referee::RefereeView,
            Animator::new(clips.get(crate::referee::CRADLE)),
            Visibility::Hidden,
            scoped.clone(),
        ));
    }
    let ball = street_render::props::spawn(
        &mut commands,
        &disc.ball.0,
        &disc.ball.1,
        Transform::from_scale(Vec3::splat(sim.world.ball.radius / (0.5 * 0.75))),
        &mut meshes,
        &mut materials,
        &mut images,
    );
    commands.entity(ball).insert((
        view::BallView,
        street_render::shadow::ShadowSource::rigid(disc.ball.0.meshes.iter()),
        street_render::net::NetCollider {
            radius: sim.world.ball.radius,
        },
        scoped.clone(),
    ));
    if let Some(crowd) = &scene.crowd {
        let root = street_render::crowd::spawn(
            &mut commands,
            crowd,
            &mut meshes,
            &mut materials,
            &mut images,
        );
        commands.entity(root).insert(scoped.clone());
    }
    // The nets, in the court's own net texture (`<court>n.ssh`, image `net1`).
    for hoop in &sim.world.hoops {
        let net = street_render::net::spawn(
            &mut commands,
            Vec3::from_array(hoop.rim),
            scene.court.textures.get("net1"),
            &mut meshes,
            &mut materials,
            &mut images,
        );
        commands.entity(net).insert(scoped.clone());
    }
    view::spawn_indicator(&mut commands, &mut meshes, &mut materials);
    view::spawn_camera(&mut commands);
}

/// The hoops' give on screen ([`street_sim::rim`]): each hoop's meshes
/// ride its bones, each net's top ring its rim.
fn show_rims(
    sim: Res<Sim>,
    mut skins: Query<&mut street_render::court::HoopSkin>,
    mut nets: Query<&mut street_render::net::Net>,
    mut meshes: ResMut<Assets<Mesh>>,
) {
    use street_render::court::BoneMove;
    let shown = |m: street_sim::rim::Move| BoneMove {
        rot: Quat::from_array(m.rot),
        from: Vec3::from_array(m.from),
        to: Vec3::from_array(m.to),
    };
    let w = &sim.world;
    let moves = [0, 1].map(|s| w.rims[s].moves(&w.rim_rigs[s]));
    for mut skin in &mut skins {
        let side = skin.side;
        skin.pose(moves[side].map(shown), &mut meshes);
    }
    for mut net in &mut nets {
        let side = net.side();
        net.rim = shown(moves[side][4].to_court(side));
    }
}

fn spawn_overlays(mut commands: Commands) {
    commands.spawn((
        Text::new(""),
        TextFont {
            font_size: bevy::text::FontSize::Px(20.0),
            ..default()
        },
        TextLayout::justify(Justify::Left),
        Node {
            position_type: PositionType::Absolute,
            top: Val::Percent(14.0),
            left: Val::Percent(4.0),
            width: Val::Percent(92.0),
            padding: UiRect::all(Val::Px(20.0)),
            ..default()
        },
        BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.75)),
        Visibility::Hidden,
        DespawnOnExit(Screen::Playing),
        ResultsText,
    ));
}

/// The crowd's calls, as the audio listener makes them: its crowd call
/// `FUN_001a75a8` reacts a share of the crowd's groups
/// (`street_render::crowd::fraction`, the leading score over the points
/// to win) through `FUN_0030d618` → `FUN_0030ca10`, a cheer for every
/// event but 0x29, a gasp for 0x29. Its callers, by the rewrite's events
/// (the same listener slots `sfx.rs` and `audio.rs` map):
///
/// | Event | Listener | Call |
/// |---|---|---|
/// | a basket | slot 26 `FUN_001a3b10` (via `FUN_001a7708`) | 0x27 |
/// | a gamebreaker basket, also | `FUN_001a3b10` → `FUN_001a6b80` (+0xc set) | 0x27 |
/// | a dunk or alley-oop (the slam `sfx.rs` folds into the basket) | slot 39 `FUN_001a5d50` | 0x25 |
/// | a block (a goaltend, as `sfx.rs` hears it) | slot 16 `FUN_001a4ce8` | 0x27 |
/// | a held-ball steal or a counter | slot 15 `FUN_001a63e0` | 0x27 |
/// | a loose ball picked up | slot 17 `FUN_001a65c0` | 0x27 |
/// | a trick move lands | slot 13 `FUN_001a5a78` | 0x29 |
///
/// Not wired: slot 76's floor contacts (`FUN_001a5348`, 0x29, sampled from
/// posed joints in `sfx.rs`), slot 22 (`FUN_001a5d10`, 0x25, sender not
/// traced) and intercepted passes (whether they reach slot 15 is not
/// traced). A replay's updates call nothing.
fn crowd_reactions(
    tick: Res<TickEvents>,
    sim: Res<Sim>,
    mut calls: ResMut<street_render::crowd::CrowdReactions>,
) {
    use street_sim::Event::*;
    if tick.replay {
        return;
    }
    let w = &sim.world;
    let fraction = crowd_fraction(w);
    for event in &tick.events {
        let (cheers, gasps) = match *event {
            // The tip-off's take (slot 22, `0x1a5d10`, 0x25).
            _ if crate::sfx::tip_taken(w, event) => (1, 0),
            Score { basket, .. } => match basket {
                Basket::Gamebreaker { .. } => (3, 0),
                Basket::Dunk { .. } | Basket::AlleyOop { .. } => (2, 0),
                _ => (1, 0),
            },
            Block { .. }
            | Goaltend { .. }
            | Steal {
                intercepted_pass: false,
                ..
            }
            | Counter { .. }
            | Rebound { .. } => (1, 0),
            Trick { .. } => (0, 1),
            _ => (0, 0),
        };
        let call = |cheer| street_render::crowd::Reaction { fraction, cheer };
        calls.0.extend((0..cheers).map(|_| call(true)));
        calls.0.extend((0..gasps).map(|_| call(false)));
    }
}

/// `FUN_001a75a8`'s share of the crowd: `FUN_001a7748`'s lead over the
/// points to win, read as `song.rs` reads the same pair for `0x1a3b10`.
/// The floor-contact call (`sfx.rs`, slot 76) shares it.
pub(crate) fn crowd_fraction(w: &street_sim::World) -> f32 {
    let (lead, to_win) = match w.rules.goal {
        Goal::Points(n) => (w.score[0].max(w.score[1]), n),
        Goal::TrickPoints(n) => (w.teams[0].trick_points.max(w.teams[1].trick_points), n),
        Goal::Gamebreaker(_) => (0, 0),
    };
    street_render::crowd::fraction(lead, to_win)
}

pub(crate) fn leave(
    mut time: ResMut<Time<Virtual>>,
    sim: Option<Res<Sim>>,
    session: Option<ResMut<crate::session::Session>>,
) {
    time.unpause();
    // The generator runs on from where the match left it, into the next
    // (a rematch's too: `session.rs`).
    if let (Some(sim), Some(mut session)) = (sim, session) {
        session.carry_on(sim.world.generator());
    }
}

/// One tick of the game, and its events into the play-by-play and box score.
/// The replay manager ([`crate::tape`]) picks a basket's instant replay.
#[allow(clippy::too_many_arguments)]
pub fn step_sim(
    mut sim: ResMut<Sim>,
    mut tick_events: ResMut<TickEvents>,
    mut stats: ResMut<Stats>,
    running: Res<crate::story::Running>,
    mut save: ResMut<crate::story::Save>,
    mut lesson_passed: ResMut<LessonPassed>,
    mut lesson_run: ResMut<school::LessonRun>,
    setup: Res<crate::setup::MatchSetup>,
    names: Res<PlayerNames>,
    teams: Res<TeamNames>,
    tape: Option<Res<crate::tape::Tape>>,
    log: Option<ResMut<EventLog>>,
    mut menu: Option<ResMut<crate::school_menu::LessonMenu>>,
    (mut prompt, mut page): (
        Option<ResMut<crate::school_prompt::Prompt>>,
        Option<ResMut<crate::school_prompt::Page>>,
    ),
    (mut level2, hud, cheats): (
        Option<ResMut<crate::setup::Level2At>>,
        Option<Res<crate::hud::HudState>>,
        Option<Res<crate::setup::MatchCheats>>,
    ),
) {
    tick_events.events.clear();
    // The lesson menu and a tutorial page hold the School's gameplay
    // (`FUN_002b9988`, `FUN_002bb338`: `FUN_0033d380(0x19d2)` suspends its
    // task groups until a lesson launches or the page closes).
    if menu.as_ref().is_some_and(|m| m.open) || page.as_ref().is_some_and(|p| p.open.is_some()) {
        return;
    }
    // The replay manager holds the gameplay while a replay plays
    // (FUN_0033d380(0x401b2), crate::tape), and a staged script's end
    // after its replay takes an update of its own.
    tick_events.replay = tape.as_ref().is_some_and(|t| t.holds_world());
    if tick_events.replay {
        return;
    }
    // A staged Level 2's task holds the game (`FUN_00144df0` suspends the
    // task groups 0x401d2) until GB2's burst begins its wipe
    // (`FUN_002bb818`), unless the No HUD cheat is on; its script starts
    // then (the School's Level 2 grabs: the game's count stands through
    // the burst).
    if sim.world.staged_start.is_some()
        && hud.as_ref().is_some_and(|h| {
            h.banner.mode == crate::banner::Mode::Gamebreaker2 && !h.banner.burst_wiping()
        })
        && !cheats.as_ref().is_some_and(|c| c.0.no_hud())
    {
        return;
    }
    if let Some(level2) = level2.as_mut() {
        level2.drive(&mut sim);
    }
    let inputs = std::mem::take(&mut sim.inputs);
    let school_before = school::Before::new(&sim.world);
    sim.world.step(&inputs);
    sim.inputs = inputs;
    if let Some(level2) = level2.as_mut() {
        level2.fill(&mut sim);
    }
    let events = sim.world.events.clone();
    tick_events.events = events.clone();
    if let Some(mut log) = log {
        log.note(&sim.world, &events);
    }

    // Native School callbacks queue a delayed outcome; only its commit
    // writes the selected profile. The lesson menu then comes back on the
    // lesson (the manager's message `0x3c6960`, `FUN_002b9988`); with
    // every lesson done and the user not yet paid (`+0x54` bit 0, once a
    // game, `+0x200`) the game ends on the completion instead.
    let mut committed = None;
    if lesson_passed.0.is_none()
        && let Some(story::Run::School(lesson)) = running.0
    {
        lesson_run.set_seen(school::pages_seen(&save.story));
        committed = lesson_run.advance(lesson, school_before, &sim.world);
        // What the manager posted: the prompt, a tutorial page, the shot
        // clock refilled, a meter emptied at the commit.
        for &post in lesson_run.posts() {
            use street_sim::school::Post;
            match post {
                Post::Prompt(record) => {
                    if let Some(p) = prompt.as_mut() {
                        p.post(record, lesson);
                    }
                }
                Post::Hide => {
                    if let Some(p) = prompt.as_mut() {
                        p.hide();
                    }
                }
                Post::Page(n) => {
                    school::see_page(&mut save.story, n);
                    save.store();
                    if let Some(p) = page.as_mut() {
                        p.open(n);
                    }
                    // `FUN_00130b40`: the pads' presses start over.
                    let held = sim.inputs.clone();
                    sim.world.settle_presses(&held);
                }
                Post::RefillClock => sim.world.refill_shot_clock(),
                Post::EmptyMeter(team) => sim.world.set_meter(team, 0.0),
            }
        }
    }
    if let (Some(story::Run::School(lesson)), Some(passed)) = (running.0, committed) {
        school::finish_for_setup(&mut save.story, &setup, lesson, passed);
        save.store();
        lesson_passed.0 = Some(if passed { Ok(lesson) } else { Err(()) });
        let complete = passed
            && crate::school_menu::all_done(save.story.school)
            && save.story.school_profile.flags & 1 == 0;
        // The menu hides the HUD's elements 0..62 (`FUN_002ba010`), the
        // prompt among them.
        if let Some(p) = prompt.as_mut() {
            p.hide();
        }
        match menu.as_mut() {
            Some(menu) if !complete => menu.open_on(lesson, save.story.school),
            _ => sim.world.phase = street_sim::Phase::Over { winner: 0 },
        }
    }

    note_events(&mut sim, &mut stats, events, &names, &teams);
}

/// A tick's events into the box score and the play-by-play.
pub(crate) fn note_events(
    sim: &mut Sim,
    stats: &mut Stats,
    events: Vec<street_sim::Event>,
    names: &PlayerNames,
    teams: &TeamNames,
) {
    // The box score is the original's statistics, which the sim keeps
    // (`street_sim::stats`, the stats listener's tallies).
    stats.sync(&sim.world.stats);
    for event in events {
        use street_sim::Event::*;
        let name = |i: usize| names.0[i].clone();
        match event {
            Pass { from, style, .. } => {
                sim.pass_style[from] = style;
                if matches!(
                    style,
                    PassStyle::Heezay | PassStyle::Glass | PassStyle::Kick
                ) {
                    stats.passouts[from] += 1;
                }
            }
            Trick {
                player,
                fakeout: Some(_),
                ..
            } => stats.fakeouts[player] += 1,
            _ => {}
        }
        let call = match event {
            Score {
                player,
                points,
                basket,
                ..
            } => match basket {
                Basket::AlleyOop { .. } => format!("ALLEY-OOP! {} throws it down", name(player)),
                Basket::Dunk { advanced: true } => {
                    format!("{} with the monster jam!", name(player))
                }
                Basket::Gamebreaker { level, .. } => {
                    format!("LEVEL {level} GAMEBREAKER by {}: {points}", name(player))
                }
                _ => format!("{} scores {points}", name(player)),
            },
            Pass {
                from,
                style: PassStyle::Heezay,
                ..
            } => format!("{}: OFF THE HEEZAY!", name(from)),
            Pass {
                from,
                style: PassStyle::Glass,
                ..
            } => format!("{}: BACK 2 PAPA!", name(from)),
            Block { player, shooter } => format!("{} blocks {}", name(player), name(shooter)),
            Steal { player, .. } => format!("{} steals it", name(player)),
            ShotClockViolation { team } => format!("Shot clock: {} lose it", teams.0[team]),
            GameOver { winner } => format!("{} win!", teams.0[winner]),
            Trick {
                player,
                fakeout: Some(victim),
                ..
            } => format!("FAKEOUT! {} breaks {}", name(player), name(victim)),
            Counter { player, from } => format!("{} counters {}", name(player), name(from)),
            Fumble { player } => format!("{} loses it", name(player)),
            Combo {
                points, multiplier, ..
            } => format!("COMBO x{multiplier}: {points} trick points"),
            GamebreakerReady { team } => format!("{}: GAMEBREAKER ready", teams.0[team]),
            Gamebreaker { player, level } => {
                format!("{}: LEVEL {level} GAMEBREAKER!", name(player))
            }
            Pocket { team } => format!("{} pocket a gamebreaker", teams.0[team]),
            Cancel { team } => format!("{} cancel the pocketed gamebreaker", teams.0[team]),
            Goaltend { player, shooter } => {
                format!("{} rejects {} at the rim!", name(player), name(shooter))
            }
            Dive { player } => format!("{} dives for it", name(player)),
            Pick { screener, defender } => {
                format!("{} picks off {}", name(screener), name(defender))
            }
            _ => continue,
        };
        info!("tick {}: {event:?}", sim.world.tick);
        sim.call = call;
    }
}

/// The rewrite's own online pause rows (the original has no online
/// pause).
const ONLINE_ITEMS: [&str; 2] = ["Keep Playing", "Leave Match"];

/// The front end's mode as the pause screen reads it (game record
/// +0x78): 1 Street School, 2 Pick Up, 4 the League Challenge, 5 Be A
/// Legend (a story event's game).
pub(crate) fn front_end_mode(running: &crate::story::Running) -> u32 {
    match running.0 {
        Some(crate::story::Run::School(_)) => 1,
        Some(crate::story::Run::League(..)) => 4,
        Some(crate::story::Run::StoryEvent(_)) => 5,
        None => 2,
    }
}

/// The Quit Game dialog's rows (`FUN_002bd540`): Yes, No and, in Pick Up
/// (modes 2 and 3), REMATCH.
fn quit_rows(gm: &crate::boards::Gm, mode: u32) -> Vec<String> {
    let mut rows = vec![gm.text(0x0002).to_owned(), gm.text(0x0003).to_owned()];
    if mode == 2 || mode == 3 {
        rows.push(gm.text(0x580e).to_owned());
    }
    rows
}

/// Start, P or Esc pauses. The pause screen ([`crate::pause`]) changes
/// the volumes and in-game options, shows the controller layout and the
/// songs, and quits (asking first) or plays again. Online nothing stops:
/// the rewrite's own two rows open over the game.
#[allow(clippy::too_many_arguments)]
fn pause(
    input: MenuInput,
    mut paused: ResMut<Paused>,
    mut running: ResMut<crate::story::Running>,
    mut time: ResMut<Time<Virtual>>,
    mut next: ResMut<NextState<Screen>>,
    sim: Res<Sim>,
    online: Option<Res<crate::online::Online>>,
    (mut sound, mut opts, mut pads, sounds): (
        ResMut<crate::audio::AudioSettings>,
        ResMut<crate::options::Options>,
        ResMut<crate::pads::PadSettings>,
        Option<Res<crate::audio::Sounds>>,
    ),
    (mut save, mut setup): (ResMut<crate::story::Save>, ResMut<setup::MatchSetup>),
    (control, mut menus, mut rematch, mut rules): (
        Res<crate::controls::Control>,
        ResMut<menu::Menus>,
        ResMut<Rematch>,
        ResMut<crate::rematch_rules::RematchRules>,
    ),
    (mut demo, real): (Option<ResMut<crate::attract::Before>>, Res<Time<Real>>),
) {
    use crate::pause::{Page, row};
    // The rematch rules over the court (`FUN_0025dc88`): their keys, the
    // game held; Continue starts it again with them.
    if let Some(d) = rules.0.as_mut() {
        // The press that opened it is not the dialog's.
        let fresh = d.ticks == 0;
        d.ticks += 1;
        let done = !fresh
            && d.press(
                input.up(),
                input.down(),
                input.left(),
                input.right(),
                input.cross(),
            );
        // Up over the court as the pause's dialogs are: the HUD hides.
        paused.open = !done;
        if done {
            d.edit.apply(&mut setup.rules);
            rules.0 = None;
            rematch.0 = true;
            next.set(Screen::Loading);
        }
        paused.on = online.is_none();
        time.pause();
        return;
    }
    // The attract's game has no pause: Start on any pad ends it
    // (`FUN_00141f18`: a press from no actor's controller sets the game's
    // `+0xf0` to -4, and the task exits two updates on, `FUN_00143180`),
    // the front end coming back through its load.
    if let Some(demo) = demo
        .as_deref_mut()
        .filter(|d| crate::attract::Before::playing(Some(d)))
    {
        // Its last frame stands until the front end's load.
        if demo.leaving.is_some() || input.pause() {
            time.pause();
            if demo.leave(real.elapsed_secs_f64()) {
                next.set(Screen::Returning);
            }
        }
        return;
    }
    let over = matches!(sim.world.phase, Phase::Over { .. });
    let held = paused.notice.is_some();
    let was_open = paused.open;
    let mode = front_end_mode(&running);
    let disabled = crate::pause::disabled(crate::pause::Context {
        mode,
        users: setup.seats.len().max(1),
        over,
    });
    // Start on a controller nobody plays with, in a Pick Up game: a
    // challenger (`FUN_002f3858`: a press from outside the match's seats
    // opens screen 100 when the front end's mode is Pick Up, 2 or 3).
    let challenger = input.paused_by().filter(|&device| {
        online.is_none()
            && running.0.is_none()
            && !control.demo
            && is_challenger(&control.seats, device)
    });
    let quit = |running: &mut crate::story::Running, next: &mut NextState<Screen>| {
        // A match abandoned before its buzzer earns nothing. The front end
        // comes back through its load (class 0x2f).
        running.0 = None;
        next.set(Screen::Returning);
    };
    let dialog_rows = if paused.asking && online.is_none() && !held && (mode == 2 || mode == 3) {
        3
    } else {
        2
    };
    if input.pause() && !over && !held && !paused.open && challenger.is_some() {
        paused.open = true;
        paused.challenge = true;
        // Continue is the default (`FUN_002676e8`).
        paused.choice = 1;
        paused.ticks = 0;
    } else if input.pause() && !over && !held && !paused.challenge && !paused.asking {
        if !paused.open {
            paused.open = true;
            paused.page = Page::List;
            paused.cursor = crate::pause::next_row(row::COUNT - 1, 1, &disabled);
            paused.choice = 0;
            paused.ticks = 0;
        } else if online.is_none() && paused.page != Page::List {
            // Start closes an open page first (`FUN_002bd038`, event
            // 0x3d), then the pause (`FUN_002bd660(0)`).
            paused.page = Page::List;
        } else {
            paused.open = false;
        }
    } else if paused.open && paused.challenge {
        if input.up() || input.down() {
            paused.choice = 1 - paused.choice.min(1);
        }
        if input.back() {
            paused.open = false;
            paused.challenge = false;
        } else if input.confirm() {
            paused.open = false;
            paused.challenge = false;
            if paused.choice == 0 {
                // Accept Challenge (`FUN_00267840` sets `0x3fa4b0`): the
                // game is left, and the front end goes back to picking
                // sides so the challenger can take one.
                running.0 = None;
                menus.set_return(Some(menu::Page::Sides));
                next.set(Screen::Returning);
            }
        }
    } else if paused.open && (online.is_some() || held || paused.asking) {
        // A dialog: its rows wrap (`FUN_0025c100`).
        if input.up() {
            paused.choice = (paused.choice + dialog_rows - 1) % dialog_rows;
        }
        if input.down() {
            paused.choice = (paused.choice + 1) % dialog_rows;
        }
        if input.back() && !held {
            if paused.asking {
                paused.asking = false;
            } else {
                paused.open = false;
            }
        } else if input.confirm() {
            if online.is_some() || held {
                // Online and while a controller is missing: keep playing
                // or leave.
                match paused.choice {
                    0 if held => {}
                    0 => paused.open = false,
                    _ => {
                        paused.open = false;
                        quit(&mut running, &mut next);
                    }
                }
            } else {
                // "Are you sure you want to quit?" (`FUN_002bd3f8`).
                match paused.choice {
                    0 => {
                        paused.open = false;
                        quit(&mut running, &mut next);
                    }
                    2 => {
                        // REMATCH (`FUN_002bd3f8`): the rematch rules over
                        // the court first (`FUN_0025dc88`), then the same
                        // game again with them (it counts once, when it
                        // ends), with no load.
                        paused.asking = false;
                        rules.0 = Some(crate::rematch_rules::RulesDialog::new(&setup.rules));
                    }
                    _ => paused.asking = false,
                }
            }
        }
    } else if paused.open {
        let step = match (input.left(), input.right()) {
            (true, false) => -1,
            (false, true) => 1,
            _ => 0,
        };
        match paused.page {
            Page::List => {
                if input.up() {
                    paused.cursor = crate::pause::next_row(paused.cursor, -1, &disabled);
                }
                if input.down() {
                    paused.cursor = crate::pause::next_row(paused.cursor, 1, &disabled);
                }
                if step != 0 && !disabled[paused.cursor] {
                    crate::pause::step(paused.cursor, step, &mut opts.0, &mut sound);
                }
                if input.back() {
                    paused.open = false;
                } else if input.confirm() && !disabled[paused.cursor] {
                    match paused.cursor {
                        row::LAYOUT => {
                            paused.page = Page::Layout;
                            // The layout is lit (`FUN_002148c8`).
                            paused.panel_cursor = 1;
                        }
                        row::TRAX => {
                            paused.page = Page::Trax;
                            paused.trax_cursor = 0;
                        }
                        row::QUIT => {
                            paused.asking = true;
                            // No is lit (`FUN_0025c6d8`).
                            paused.choice = 1;
                        }
                        _ => {}
                    }
                }
            }
            Page::Layout => {
                // The panel takes the pad's moves (`FUN_00215668`,
                // `FUN_00215618`, `FUN_002158c0`, `FUN_002156c8`): up and
                // down between the layout (1) and the view (2), left and
                // right change them; Circle switches vibration
                // (`func_0x002149f8`); Start closes the panel.
                if input.up() || input.down() {
                    paused.panel_cursor = if paused.panel_cursor == 1 { 2 } else { 1 };
                }
                let player = pausing_player(&control);
                if step != 0 {
                    if paused.panel_cursor == 1 {
                        let l = &mut opts.0.layouts[player];
                        *l = (i32::from(*l) + step).rem_euclid(3) as u8;
                    } else {
                        paused.panel_view = 1 - paused.panel_view.min(1);
                    }
                }
                if input.back() {
                    pads.vibration = !pads.vibration;
                }
            }
            Page::Trax => {
                // Music Experience, then the songs: left and right switch
                // the lit one (as the front end's Trax,
                // `story::trax_step` rows 0 and 3..).
                let songs = sounds.as_ref().map_or(0, |s| s.songs().len());
                let count = 1 + songs.min(street_sim::records::FRONT_END_BIT as usize);
                if input.up() {
                    paused.trax_cursor = (paused.trax_cursor + count - 1) % count;
                }
                if input.down() {
                    paused.trax_cursor = (paused.trax_cursor + 1) % count;
                }
                if step != 0 {
                    let row = match paused.trax_cursor {
                        0 => 0,
                        k => k + 2,
                    };
                    crate::story::trax_step(&mut opts.0, row, songs);
                }
                if input.back() {
                    paused.page = Page::List;
                }
            }
        }
    }
    if paused.open {
        paused.ticks += 1;
    } else {
        paused.asking = false;
        paused.page = Page::List;
    }
    // Closing the menu keeps what it changed in the game data; the load
    // back's save to the card writes it (`FUN_002f2e50`'s bit 4).
    if was_open && !paused.open && online.is_none() {
        save.story.settings = Some(crate::options::gather(
            &opts.0,
            &sound,
            &pads,
            setup.rules.difficulty,
        ));
        save.store();
    }
    paused.on = paused.open && online.is_none();
    if paused.on {
        time.pause();
    } else {
        time.unpause();
    }
}

/// Street School's game opens on its lesson menu (`FUN_002b1ae8`: the
/// School's HUD focuses the menu's list on its first row), with no intro.
fn open_school_menu(
    running: Res<crate::story::Running>,
    save: Res<crate::story::Save>,
    mut menu: ResMut<crate::school_menu::LessonMenu>,
) {
    *menu = default();
    if let Some(story::Run::School(lesson)) = running.0 {
        menu.open_on(lesson, save.story.school);
    }
}

/// The lesson menu's keys (`FUN_002b3ad0`) while no lesson runs and the
/// pause is down; Cross on a lesson launches it (`FUN_002b4400`: the
/// manager's `FUN_0018ec28`, [`school::stage_world`]).
#[allow(clippy::too_many_arguments)]
fn school_menu_keys(
    input: MenuInput,
    mut menu: ResMut<crate::school_menu::LessonMenu>,
    paused: Res<Paused>,
    mut running: ResMut<crate::story::Running>,
    mut sim: ResMut<Sim>,
    mut save: ResMut<crate::story::Save>,
    (setup, control): (Res<setup::MatchSetup>, Res<crate::controls::Control>),
    (mut lesson_passed, mut lesson_run): (ResMut<LessonPassed>, ResMut<school::LessonRun>),
    mut prompt: ResMut<crate::school_prompt::Prompt>,
) {
    use crate::school_menu::Key;
    if !menu.open || paused.open() {
        return;
    }
    if !matches!(running.0, Some(story::Run::School(_))) {
        menu.open = false;
        return;
    }
    menu.ticks += 1;
    let key = if input.cross() {
        Key::Cross
    } else if input.back() && !input.pause() {
        Key::Triangle
    } else if input.shoulder() != 0 {
        Key::Shoulder(input.shoulder())
    } else if input.up() {
        Key::Step(-1)
    } else if input.down() {
        Key::Step(1)
    } else {
        return;
    };
    let done = save.story.school;
    if let Some(lesson) = menu.key(key, done) {
        school::start_for_setup(&mut save.story, &setup, lesson);
        save.store();
        running.0 = Some(story::Run::School(lesson));
        let selected = control.first_player().unwrap_or(0);
        let held = sim.inputs.clone();
        school::stage_world(
            &mut sim.world,
            lesson,
            selected,
            school::pages_seen(&save.story),
            &held,
        );
        // `FUN_0018ec28` starts the pads' presses over (`FUN_00130b40`):
        // the Cross that launched it is no pass.
        let held = sim.inputs.clone();
        sim.world.settle_presses(&held);
        lesson_passed.0 = None;
        *lesson_run = school::LessonRun::default();
        prompt.launch(lesson);
        menu.open = false;
    }
}

/// A tutorial page's keys: Cross closes it and the School's game goes on
/// (its presses started over, `FUN_00130b40`).
fn school_page_keys(
    input: MenuInput,
    paused: Res<Paused>,
    mut page: ResMut<crate::school_prompt::Page>,
    mut sim: ResMut<Sim>,
) {
    if page.open.is_none() || paused.open() {
        return;
    }
    if input.cross() {
        page.open = None;
        let held = sim.inputs.clone();
        sim.world.settle_presses(&held);
    }
}

/// The HUD's update for the lesson prompt's fade (`FUN_002b4c38`) and the
/// open page's own (`FUN_00253088`'s count).
fn school_prompt_step(
    mut prompt: ResMut<crate::school_prompt::Prompt>,
    mut page: ResMut<crate::school_prompt::Page>,
) {
    prompt.tick();
    if page.open.is_some() {
        page.count.step();
    }
}

/// Draws the lesson menu over the School's game; it hides under the pause
/// as the HUD does (`FUN_002b38a0`).
#[allow(clippy::too_many_arguments)]
fn draw_school_menu(
    menu: Res<crate::school_menu::LessonMenu>,
    page: Res<crate::school_prompt::Page>,
    paused: Res<Paused>,
    save: Res<crate::story::Save>,
    (setup, disc, hud_art): (
        Res<setup::MatchSetup>,
        Res<setup::Disc>,
        Option<Res<crate::hud::HudArt>>,
    ),
    (look, gm, art): (
        Option<Res<crate::look::Look>>,
        Option<Res<crate::boards::Gm>>,
        Option<Res<crate::art::Art>>,
    ),
    mut labels: ResMut<crate::look::Labels>,
    mut images: ResMut<Assets<Image>>,
    mut panels: Local<crate::boards::Panels>,
    mut commands: Commands,
    mut slots: Query<(
        &crate::school_menu::MenuSlot,
        &mut ImageNode,
        &mut Node,
        &mut Visibility,
    )>,
) {
    let mut out = Vec::new();
    if let (Some(_), false, Some(look), Some(gm)) =
        (page.open, paused.open(), look.as_deref(), gm.as_deref())
    {
        // The Lakers' logo for the pages' HUD panel (the School's left).
        let logo = disc
            .teams
            .get(setup.teams[1])
            .and_then(|t| usize::try_from(t.logo).ok())
            .and_then(|l| hud_art.as_deref()?.logos.get(l)?[0].clone());
        out = crate::school_prompt::page_pieces(
            look,
            gm,
            &mut labels,
            &mut images,
            &mut panels,
            &page,
            crate::school_prompt::PagePicture {
                atlas: art.as_deref().and_then(|a| a.hud.as_ref()),
                logo,
                indicator: art.as_deref().and_then(|a| a.indicator.as_ref()),
            },
        );
    } else if let (true, false, Some(look), Some(gm)) =
        (menu.open, paused.open(), look.as_deref(), gm.as_deref())
    {
        out = crate::school_menu::pieces(
            look,
            gm,
            &mut labels,
            &mut images,
            &mut panels,
            art.as_deref().and_then(|a| a.school.as_ref()),
            &menu,
            save.story.school,
        );
    }
    crate::school_menu::show(&mut commands, &mut slots, &out);
}

/// The first seated player's slot (the pause screen is the pausing
/// pad's, `DAT_003fef04`; the rewrite keeps the first seat's).
fn pausing_player(control: &crate::controls::Control) -> usize {
    control
        .seats
        .iter()
        .position(|s| s.device.is_some())
        .unwrap_or(0)
        .min(3)
}

/// Draws the pause screen and its dialogs ([`crate::pause`]) over the
/// match; the HUD hides under them ([`crate::hud::draw`]).
#[allow(clippy::too_many_arguments)]
fn draw_pause(
    paused: Res<Paused>,
    (look, gm): (
        Option<Res<crate::look::Look>>,
        Option<Res<crate::boards::Gm>>,
    ),
    mut labels: ResMut<crate::look::Labels>,
    mut images: ResMut<Assets<Image>>,
    (opts, sound, pads, sounds): (
        Res<crate::options::Options>,
        Res<crate::audio::AudioSettings>,
        Res<crate::pads::PadSettings>,
        Option<Res<crate::audio::Sounds>>,
    ),
    (disc, running, control, sim, setup): (
        Res<setup::Disc>,
        Res<crate::story::Running>,
        Res<crate::controls::Control>,
        Res<Sim>,
        Res<setup::MatchSetup>,
    ),
    (online, rules): (
        Option<Res<crate::online::Online>>,
        Res<crate::rematch_rules::RematchRules>,
    ),
    mut panels: Local<crate::boards::Panels>,
    mut commands: Commands,
    mut slots: Query<(
        &crate::pause::PauseSlot,
        &mut ImageNode,
        &mut Node,
        &mut Visibility,
    )>,
) {
    use crate::pause::{Dialog, Page, View};
    let mut out = Vec::new();
    if let (Some(d), Some(look), Some(gm)) = (rules.0.as_ref(), look.as_deref(), gm.as_deref()) {
        labels.tidy();
        // The sides' people (the rewrite's side 0 is home), "CPU" for a
        // side with nobody.
        let people: Vec<(usize, Option<String>)> = setup
            .seats
            .iter()
            .enumerate()
            .map(|(k, _)| (k, setup.users.get(k).cloned().flatten()))
            .collect();
        let side_of = |k: usize| setup.seats.get(k).is_some_and(|&(_, s)| s == 0);
        let away = crate::myrules::side_names(look, &people, false, side_of);
        let home = crate::myrules::side_names(look, &people, true, side_of);
        out = crate::rematch_rules::pieces(
            look,
            gm,
            &mut labels,
            &mut images,
            &mut panels,
            d,
            [&away, &home],
        );
    } else if let (true, Some(look), Some(gm)) = (paused.open, look.as_deref(), gm.as_deref()) {
        labels.tidy();
        let mode = front_end_mode(&running);
        let disabled = crate::pause::disabled(crate::pause::Context {
            mode,
            users: setup.seats.len().max(1),
            over: matches!(sim.world.phase, Phase::Over { .. }),
        });
        let mut dialog = |text: &str, rows: &[String], dim: f32| {
            crate::pause::dialog(
                look,
                gm,
                &mut labels,
                &mut images,
                &mut panels,
                &Dialog {
                    text,
                    rows,
                    row_font: crate::pause::FONT,
                    lit: paused.choice,
                    ticks: paused.ticks,
                    dim,
                    width: 0.0,
                },
            )
        };
        if paused.challenge {
            // `challengercomes.txt` (0x64): the question, Accept
            // Challenge, Continue.
            let rows = [gm.text(0x6401).to_owned(), gm.text(0x6402).to_owned()];
            out = dialog(gm.text(0x6400), &rows, 0.0);
        } else if let Some(why) = paused.notice {
            // The rewrite's own: the original's held-pause notice is not
            // traced.
            let rows = [gm.text(0x580a).to_owned(), gm.text(0x580c).to_owned()];
            out = dialog(why, &rows, 0.0);
        } else if online.is_some() {
            let rows = ONLINE_ITEMS.map(String::from);
            out = dialog("", &rows, 0.0);
        } else {
            let trax = if paused.page == Page::Trax {
                let songs = sounds.as_ref().map_or(&[][..], |s| s.songs());
                crate::pause::trax_rows(&opts.0, songs, &disc.radio)
            } else {
                Default::default()
            };
            let player = pausing_player(&control);
            let view = View {
                page: paused.page,
                cursor: paused.cursor,
                settings: &opts.0,
                sound: &sound,
                ticks: paused.ticks,
                controller: crate::pause::Controller {
                    layout: opts.0.layouts[player].min(2),
                    player,
                    view: paused.panel_view,
                    cursor: paused.panel_cursor,
                    vibration: pads.vibration,
                },
                trax: &trax,
                trax_cursor: paused.trax_cursor,
                disabled,
            };
            out = crate::pause::pieces(look, gm, &mut labels, &mut images, &mut panels, &view);
            if paused.asking {
                let rows = quit_rows(gm, mode);
                out.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut labels,
                    &mut images,
                    &mut panels,
                    &Dialog {
                        text: gm.text(0x580d),
                        rows: &rows,
                        row_font: crate::pause::FONT,
                        lit: paused.choice,
                        ticks: paused.ticks,
                        dim: crate::pause::PAUSE_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
    }
    crate::pause::show(&mut commands, &mut slots, &out);
}

/// A controller that has no seat in the match (a seat on every controller
/// counts every one).
fn is_challenger(seats: &[controls::Seat], device: controls::Device) -> bool {
    !seats
        .iter()
        .any(|s| s.device.is_none() || s.device == Some(device))
}

/// After the final buzzer: the result and the box score, what the story
/// paid out, then back to the menus. The boards are drawn by
/// [`crate::boards`] on the game's own layouts; without them, as text.
#[allow(clippy::too_many_arguments)]
fn results(
    time: Res<Time>,
    input: MenuInput,
    sim: Res<Sim>,
    stats: Res<Stats>,
    mut running: ResMut<crate::story::Running>,
    mut save: ResMut<crate::story::Save>,
    mut lesson_passed: ResMut<LessonPassed>,
    mut menus: ResMut<menu::Menus>,
    names: Res<PlayerNames>,
    teams: Res<TeamNames>,
    (setup, disc, mut post, look, gm): (
        Res<setup::MatchSetup>,
        Res<setup::Disc>,
        ResMut<crate::boards::PostGame>,
        Option<Res<crate::look::Look>>,
        Option<Res<crate::boards::Gm>>,
    ),
    (mut finished, mut earned, tape, mut after): (
        ResMut<Finished>,
        Local<Option<Vec<String>>>,
        Option<Res<crate::tape::Tape>>,
        ResMut<AfterGame>,
    ),
    (mut screen, mut choice, mut was_pickup): (Local<usize>, Local<usize>, Local<bool>),
    (mut online, mut rules, mut demo, real): (
        Option<ResMut<crate::online::Online>>,
        ResMut<crate::rematch_rules::RematchRules>,
        Option<ResMut<crate::attract::Before>>,
        Res<Time<Real>>,
    ),
    mut next: ResMut<NextState<Screen>>,
    mut text: Query<(&mut Text, &mut Visibility), With<ResultsText>>,
) {
    let Phase::Over { winner } = sim.world.phase else {
        return;
    };
    // The rematch rules are up over the court (the pause's keys run
    // them): the post-game dialog is gone.
    if rules.0.is_some() {
        post.options = Vec::new();
        return;
    }
    // The won game's replay plays first (FUN_003248d8's forced replay; the
    // game listener's replay end, FUN_00142648, moves phase 9 on). The
    // seconds after it are the rewrite's own.
    if tape.as_ref().is_some_and(|t| t.holding()) {
        return;
    }
    // The attract's game ends with that replay: no boards, the front end
    // back through its load (`FUN_002f3918`: mode 0 exits the task).
    if let Some(demo) = demo
        .as_deref_mut()
        .filter(|d| crate::attract::Before::playing(Some(d)))
    {
        if demo.leave(real.elapsed_secs_f64()) {
            next.set(Screen::Returning);
        }
        return;
    }
    finished.0 += time.delta_secs();
    if finished.0 < 2.5 {
        // A new game's buzzer: its own results (the last game's go).
        *earned = None;
        *screen = 0;
        *choice = 0;
        if post.active() {
            *post = crate::boards::PostGame::default();
        }
        return;
    }
    // What the story makes of this game, once: the counters in, the
    // notices out, kept for the card's next save.
    if earned.is_none() {
        after.finished = true;
        // `FUN_001ded40` hands the story the side's stats 0xb, 0xc and 0xd:
        // its trick points and Level 1 and Level 2 gamebreaker baskets. The
        // story modes' person plays side 1 (`story::CAREER_SIDE`).
        use street_sim::stats::{LEVEL1, LEVEL2, TRICK_POINTS};
        let mine = match running.0 {
            Some(story::Run::StoryEvent(_) | story::Run::League(..)) => story::CAREER_SIDE,
            _ => 0,
        };
        let side = |stat: usize| sim.world.stats.team(mine, stat).max(0) as u32;
        let outcome = street_sim::story::MatchOutcome {
            won: winner == mine,
            trick_points: side(TRICK_POINTS),
            level1_gamebreakers: side(LEVEL1).min(255) as u8,
            level2_gamebreakers: side(LEVEL2).min(255) as u8,
            difficulty: match setup.rules.difficulty {
                street_sim::Difficulty::Easy => 0,
                street_sim::Difficulty::Medium => 1,
                street_sim::Difficulty::Hard => 2,
            },
        };
        let mut lines: Vec<String> = Vec::new();
        // What the boards show themselves, and the rest.
        let mut shown: Vec<street_sim::story::Notice> = Vec::new();
        let mut notes: Vec<String> = Vec::new();
        let mut kind = crate::boards::Kind::Match;
        let reputation_before = save.story.reputation;
        let lesson = matches!(running.0, Some(story::Run::School(_)));
        let pickup = running.0.is_none();
        *was_pickup = pickup && !lesson && !setup.online;
        *screen = 0;
        *choice = 0;
        let legend_before = save.story.legend_complete();
        match running.0.take() {
            Some(story::Run::School(_)) | None => {
                // A Street School lesson's practice game, or a plain match.
                match lesson_passed.0.take() {
                    Some(Ok(lesson)) => {
                        // Every lesson done: the School's game ends on its
                        // completion, back to Play It's Street School.
                        kind = crate::boards::Kind::Lesson;
                        lines.push(format!(
                            "LESSON COMPLETE: {}",
                            school::LESSONS[lesson].title
                        ));
                        menus.set_return(Some(menu::Page::PlayIt));
                    }
                    Some(Err(())) => {
                        // The lesson's native failure code was recorded during play.
                        kind = crate::boards::Kind::Lesson;
                        lines.push("Lesson incomplete. Try again.".into());
                        menus.set_return(Some(menu::Page::PlayIt));
                    }
                    None => {
                        menus.set_return(None);
                        // A Pick Up game a person won pays Reward Points
                        // (not a demo or a match over the network).
                        let people = if setup.seats.is_empty() {
                            vec![0]
                        } else {
                            setup.seats.iter().map(|&(_, side)| side).collect()
                        };
                        if pickup
                            && !lesson
                            && !setup.demo
                            && !setup.online
                            && people.contains(&winner)
                        {
                            let rp = save.story.win_pickup(outcome.difficulty);
                            shown.push(street_sim::story::Notice::RewardPoints(rp));
                            for id in crate::rewards::pickup_rewards(&mut save.story, &disc.cards) {
                                if let Some(card) = disc.cards.iter().find(|c| c.id as u16 == id) {
                                    // A Street Legends card goes through the
                                    // completion's notice (kind 10); the rest
                                    // are granted with none (0x1df6b0..).
                                    if card.category == crate::rewards::STREET_LEGENDS {
                                        shown.push(street_sim::story::Notice::Card(id));
                                    }
                                    lines.push(format!(
                                        "Unlocked: {}",
                                        crate::rewards::card_name(&disc.poster, card)
                                    ));
                                }
                            }
                            lines.push(format!(
                                "+{rp} Reward Points ({} total)",
                                save.story.reward_points
                            ));
                        }
                    }
                }
            }
            Some(crate::story::Run::StoryEvent(ev)) => {
                kind = crate::boards::Kind::Legend(ev);
                let mut earned_notes = save.story.play(ev, &outcome);
                // The event's court, once it is cleared (kind 2).
                if let Some(court) = crate::rewards::grant_event_court(&mut save.story, &disc, ev) {
                    earned_notes.insert(0, court);
                }
                for note in &earned_notes {
                    let line = match note {
                        street_sim::story::Notice::EventOpen(e) => {
                            crate::story::unlocked_line(&disc, usize::from(*e))
                        }
                        street_sim::story::Notice::Milestone(n) => {
                            crate::rewards::grant_milestone(&mut save.story, &disc, *n)
                        }
                        street_sim::story::Notice::Nickname => {
                            crate::story::give_nickname(&mut save.story, &disc)
                        }
                        _ => describe_notice(note, &save.story),
                    };
                    if crate::boards::on_board(note) {
                        shown.push(*note);
                    } else {
                        notes.push(line.clone());
                    }
                    lines.push(line);
                }
                // The Street Champ jersey, from the Legendary Challenge's
                // clearing until the completion board is passed (the
                // board follows the game that completes the mode).
                let champ = street_sim::story::EVENTS
                    .iter()
                    .position(|e| e.court == 10)
                    .is_some_and(|e| save.story.cleared(e));
                if champ && !legend_before {
                    shown.push(street_sim::story::Notice::StreetChamp);
                }
                // A tournament goes back to its bracket.
                let tournament =
                    street_sim::story::EVENTS[ev].kind == street_sim::story::EventKind::Tournament;
                menus.set_return(Some(if tournament {
                    menu::Page::Bracket(ev)
                } else {
                    menu::Page::City(street_sim::story::city_of(ev))
                }));
                // A win may let a beaten player join (Update Team). A
                // tournament's title shows the final standings first, and
                // Cross there opens Update Team (`FUN_0029d5a8` leaves
                // tournaments to their bracket, `FUN_002cd0f8`).
                let beaten: Vec<u16> = setup.lineups[story::OPPONENT_SIDE]
                    .iter()
                    .map(|&r| r as u16)
                    .collect();
                if save.story.offers_update(ev, outcome.won, &beaten) {
                    menus.offer_update(ev, beaten);
                    if tournament {
                        menus.set_return(Some(menu::Page::Bracket(ev)));
                    }
                }
            }
            Some(crate::story::Run::League(region, index)) => {
                kind = crate::boards::Kind::League;
                let was_done = save.story.complete_league();
                let earned_notes = crate::story::play_league_for_setup(
                    &mut save.story,
                    &setup,
                    region,
                    index,
                    &outcome,
                );
                if earned_notes
                    .iter()
                    .any(|note| matches!(note, street_sim::story::Notice::Court(_)))
                {
                    crate::rewards::grant_league_region(&mut save.story, &disc, region);
                }
                for note in &earned_notes {
                    let line = match note {
                        street_sim::story::Notice::Milestone(n) => {
                            crate::rewards::grant_milestone(&mut save.story, &disc, *n)
                        }
                        _ => describe_notice(note, &save.story),
                    };
                    if crate::boards::on_board(note) {
                        shown.push(*note);
                    } else {
                        notes.push(line.clone());
                    }
                    lines.push(line);
                }
                // The whole challenge done: its end board.
                let done = !was_done && save.story.complete_league();
                menus.set_return(Some(if done {
                    menu::Page::Completed(0)
                } else {
                    menu::Page::LeagueRegion(region)
                }));
            }
        }
        // The game in the users' records and the High Scores (not a
        // lesson's practice).
        if !lesson && lesson_passed.0.is_none() {
            crate::highscores::record_match(&mut save.story, &sim.world, &stats, &setup, winner);
        }
        // Street Milestones the game itself completes, then each one's
        // throwback jersey.
        let mut reached = crate::rewards::game_milestones(
            &mut save.story,
            &sim.world,
            &stats,
            &setup,
            winner,
            (lesson, pickup),
        );
        if lesson {
            reached.extend(school::completion_rewards_for_setup(
                &mut save.story,
                &setup,
            ));
        }
        for note in &reached {
            match *note {
                street_sim::story::Notice::Milestone(n) => {
                    let line = crate::rewards::grant_milestone(&mut save.story, &disc, n);
                    shown.push(*note);
                    lines.push(line);
                }
                street_sim::story::Notice::RewardPoints(_) => {
                    shown.push(*note);
                    lines.push(describe_notice(note, &save.story));
                }
                _ => {}
            }
        }
        // Be A Legend finished: the crew's star joins the Street Legends,
        // and the mode's end board follows.
        if !legend_before && save.story.legend_complete() {
            menus.set_return(Some(menu::Page::Completed(1)));
            let star = names.0.first().cloned().unwrap_or_default();
            let games = save.story.games;
            // `FUN_001d4438`'s mode-5 row: the star's names, the nickname
            // the career earned and its record.
            let row = street_sim::records::Legend {
                name: star.clone(),
                nickname: crate::story::nickname_of(&save.story, &disc).unwrap_or_default(),
                wins: save.story.wins,
                losses: save.story.losses,
            };
            save.story.records.legend(row);
            lines.push(format!("STREET LEGEND: {star} in {games} games"));
            notes.push(lines.last().cloned().unwrap_or_default());
        }
        // The milestones' jerseys that bring a boss move (`FUN_001e0910`),
        // in the order the milestones came.
        let milestone_moves: Vec<_> = shown
            .iter()
            .filter_map(|n| match *n {
                street_sim::story::Notice::Milestone(m) => {
                    crate::rewards::milestone_boss_move(&disc, m)
                }
                _ => None,
            })
            .collect();
        for note in milestone_moves {
            lines.push(describe_notice(&note, &save.story));
            shown.push(note);
        }
        // The fourth trophy, after any game that completes it.
        for note in save.story.trophy_rewards() {
            lines.push(describe_notice(&note, &save.story));
            shown.push(note);
        }
        if kind == crate::boards::Kind::Lesson {
            notes = lines.clone();
        }
        save.store();
        let report = crate::boards::Report {
            kind,
            shown,
            notes,
            reputation: save.story.reputation.cmp(&reputation_before),
            star: save.story.crew.first().map(|&r| usize::from(r)),
            nickname: usize::try_from(save.story.nickname_id).ok(),
        };
        post.boards = crate::boards::plan(&report, &disc, look.as_deref(), gm.as_deref());
        *earned = Some(lines);
    }
    let extra = earned.as_ref().cloned().unwrap_or_default();
    // The game's boards when its layouts are there, else text pages.
    let pages = if post.active() {
        Vec::new()
    } else {
        postgame_pages(&sim.world, &stats, &names.0, &teams.0, winner, &extra)
    };
    let last = if post.active() {
        post.boards.len()
    } else {
        pages.len()
    };
    // After the pages, what next (`common_gm.txt` 9, 10, 28, 30): a Pick
    // Up game can be played again or its teams picked anew; an online game
    // votes to replay the round with everyone else.
    let may_vote = online.as_ref().is_some_and(|o| o.can_rematch());
    let voted = online.as_ref().is_some_and(|o| o.rematch_voted());
    let options: Vec<&str> = if *was_pickup {
        // A Pick Up game's choices (`FUN_002d4ed0`: `common_gm.txt` 10,
        // 30, 28, 29; REMATCH first and lit).
        vec![
            "REMATCH",
            "QUIT TO CHOOSE TEAM",
            "QUIT TO MAIN MENU",
            "QUIT TO REWARDS",
        ]
    } else if setup.online && may_vote {
        vec!["CONTINUE", "REMATCH"]
    } else {
        vec!["CONTINUE"]
    };
    let mut lines = if let Some(page) = pages.get(*screen) {
        page.clone()
    } else {
        let mut l = vec![
            format!("{} WIN", teams.0[winner].to_uppercase()),
            String::new(),
        ];
        for (k, o) in options.iter().enumerate() {
            l.push(format!("{} {o}", if k == *choice { ">" } else { " " }));
        }
        l
    };
    lines.push(String::new());
    if setup.online && voted {
        lines.push("Rematch: waiting for the other players...".into());
        lines.push(String::new());
    }
    lines.push(format!(
        "Press {} to continue   ({} of {})",
        input.style().confirm(),
        (*screen + 1).min(last + 1),
        last + 1
    ));
    if post.active() {
        post.page = *screen;
        post.options = if *screen >= last {
            options.iter().map(|o| (*o).to_owned()).collect()
        } else {
            Vec::new()
        };
        post.choice = *choice;
        post.waiting = setup.online && voted;
        for (_, mut visibility) in &mut text {
            *visibility = Visibility::Hidden;
        }
    } else {
        for (mut t, mut visibility) in &mut text {
            *visibility = Visibility::Visible;
            t.0 = lines.join("\n");
        }
    }
    if *screen >= last {
        if input.up() {
            *choice = (*choice + options.len() - 1) % options.len();
        }
        if input.down() {
            *choice = (*choice + 1) % options.len();
        }
    }
    if !input.confirm() {
        return;
    }
    if *screen < last {
        *screen += 1;
        return;
    }
    match options[*choice] {
        "REMATCH" if setup.online => {
            // The vote goes to the server; the new round comes back
            // through the lobby when everyone agrees.
            if let Some(o) = online.as_mut() {
                o.request_rematch();
            }
        }
        "REMATCH" => {
            // The rematch rules first (`FUN_0025dc88`), then the same
            // teams and court again, with no load.
            menus.set_return(None);
            rules.0 = Some(crate::rematch_rules::RulesDialog::new(&setup.rules));
        }
        "QUIT TO CHOOSE TEAM" => {
            menus.set_return(Some(menu::Page::ChooseTeam(0)));
            next.set(Screen::Returning);
        }
        "QUIT TO REWARDS" => {
            menus.set_return(Some(menu::Page::Rewards));
            next.set(Screen::Returning);
        }
        // The front end comes back through its load (class 0x2f).
        _ => next.set(Screen::Returning),
    }
}

/// The post-game screens, as the game's own (`topballer.txt`,
/// `ballerbreakdown.txt`, `rundown.txt`, the stat words of
/// `common_gm.txt` 3..8), then the rewards: Top Ballers (the leader in
/// each), Baller Breakdown (every player's line), The Rundown (each
/// team's totals), and what the game paid out.
fn postgame_pages(
    w: &SimWorld,
    stats: &Stats,
    names: &[String],
    teams: &[String],
    winner: usize,
    rewards: &[String],
) -> Vec<Vec<String>> {
    let n = w.players.len();
    let trick_points = &stats.tricks;
    let columns: [(&str, &Vec<u32>); 6] = [
        ("POINTS", &stats.points),
        ("DUNKS", &stats.dunks),
        ("BLOCKS", &stats.blocks),
        ("STEALS", &stats.steals),
        ("REBOUNDS", &stats.rebounds),
        ("TRICK POINTS", trick_points),
    ];
    let head = vec![
        format!("{} WIN", teams[winner].to_uppercase()),
        format!(
            "{} {}  -  {} {}",
            teams[0], w.score[0], w.score[1], teams[1]
        ),
        String::new(),
    ];
    // Top Ballers.
    let mut top = head.clone();
    top.push("TOP BALLERS".into());
    for (label, v) in columns {
        if let Some(best) = (0..n).max_by_key(|&i| v.get(i).copied().unwrap_or(0)) {
            top.push(format!(
                "{label:<13} {:<22} {:>8}",
                names.get(best).cloned().unwrap_or_default(),
                v.get(best).copied().unwrap_or(0)
            ));
        }
    }
    // Baller Breakdown.
    let mut breakdown = head.clone();
    breakdown.push("BALLER BREAKDOWN".into());
    breakdown.push(format!(
        "{:<20} {:>4} {:>5} {:>5} {:>5} {:>5} {:>8}",
        "", "PTS", "DNK", "BLK", "STL", "REB", "TRICKS"
    ));
    for i in 0..n {
        if i == n / 2 {
            breakdown.push(String::new());
        }
        let at = |v: &Vec<u32>| v.get(i).copied().unwrap_or(0);
        breakdown.push(format!(
            "{:<20} {:>4} {:>5} {:>5} {:>5} {:>5} {:>8}",
            names.get(i).cloned().unwrap_or_default(),
            at(&stats.points),
            at(&stats.dunks),
            at(&stats.blocks),
            at(&stats.steals),
            at(&stats.rebounds),
            at(trick_points)
        ));
    }
    // The Rundown.
    let mut rundown = head;
    rundown.push("THE RUNDOWN".into());
    let team_sum = |v: &Vec<u32>, t: usize| -> u32 {
        (0..n)
            .filter(|&i| w.team_of(i) == t)
            .map(|i| v.get(i).copied().unwrap_or(0))
            .sum()
    };
    rundown.push(format!("{:<22} {:>10} {:>10}", "", teams[0], teams[1]));
    let rows: [(&str, [u32; 2]); 6] = [
        (
            "TRICK POINTS",
            [w.teams[0].trick_points, w.teams[1].trick_points],
        ),
        (
            "LEVEL 1 GAMEBREAKERS",
            [team_sum(&stats.gb1, 0), team_sum(&stats.gb1, 1)],
        ),
        (
            "LEVEL 2 GAMEBREAKERS",
            [team_sum(&stats.gb2, 0), team_sum(&stats.gb2, 1)],
        ),
        (
            "2 POINTERS",
            [team_sum(&stats.long, 0), team_sum(&stats.long, 1)],
        ),
        (
            "DUNKS",
            [team_sum(&stats.dunks, 0), team_sum(&stats.dunks, 1)],
        ),
        (
            "REBOUNDS",
            [team_sum(&stats.rebounds, 0), team_sum(&stats.rebounds, 1)],
        ),
    ];
    for (label, [a, b]) in rows {
        rundown.push(format!("{label:<22} {a:>10} {b:>10}"));
    }
    let mut pages = vec![top, breakdown, rundown];
    if !rewards.is_empty() {
        let mut r = vec!["REWARDS".to_owned(), String::new()];
        r.extend(rewards.iter().cloned());
        pages.push(r);
    }
    pages
}

/// A post-game notice as the game's reward screens say it.
fn describe_notice(note: &street_sim::story::Notice, story: &street_sim::story::Story) -> String {
    use street_sim::story::Notice;
    match note {
        Notice::NextRound { kind, round } => format!(
            "On to the {}",
            street_sim::bracket::round_name(*kind, usize::from(*round))
        ),
        Notice::Eliminated => "Knocked out of the tournament".into(),
        Notice::EventOpen(e) => format!("Event {e} unlocked"),
        Notice::Milestone(n) => format!("Street Milestone #{}", n + 1),
        Notice::DevBreakdown(d) => format!(
            "Trick Points: {}   Level 1 Gamebreakers: {}   Level 2 Gamebreakers: {}   Completed Court: {}   Win / Loss Bonus: {}",
            d.tricks, d.level1, d.level2, d.court, d.win_loss
        ),
        Notice::Development(dp) => {
            format!("+{dp} development points ({} total)", story.star_points())
        }
        Notice::Progress(pp) => format!("+{pp} progress points"),
        Notice::Reputation(level) => format!(
            "Reputation: {} ({} of {} points)",
            crate::story::level_name(*level),
            story.reputation % street_sim::story::REP_LEVEL_SIZE,
            street_sim::story::REP_LEVEL_SIZE
        ),
        Notice::Team(team) => format!("Team unlocked in Pick Up: {team}"),
        Notice::Court(_) => "Court unlocked".into(),
        Notice::Player(_) => "A rival asked to join your crew".into(),
        Notice::Jersey(card) => format!("Jersey unlocked: card {card}"),
        Notice::BossMove(row) => format!("Boss move unlocked: {row}"),
        Notice::Card(card) => format!("Card unlocked: {card}"),
        Notice::StreetChamp => "Street Champ jersey".into(),
        Notice::RewardPoints(r) => format!("Reward Points: {r}"),
        Notice::Nickname => "A new nickname".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use controls::{Device, Seat};

    #[test]
    fn the_camera_steps_through_intros_and_replays_but_not_while_paused() {
        #[derive(Resource, Default)]
        struct Steps(u32);
        let mut app = App::new();
        app.add_plugins(bevy::state::app::StatesPlugin)
            .insert_state(Screen::Playing)
            .init_resource::<Paused>()
            .init_resource::<Steps>()
            .add_systems(
                FixedUpdate,
                camera_step(|mut steps: ResMut<Steps>| steps.0 += 1),
            );
        let part = crate::intro::Part {
            script: 0,
            plan: street_sim::gamebreaker::SequencePlan {
                script: 0,
                kind: 0,
                frame_count: 100,
                actors: vec![],
                curves: vec![],
                ball_cues: vec![],
                controls: Default::default(),
                finishes: vec![],
            },
            actors: [None; 7],
            mirrored: false,
            hoop: 0,
            cast_only: true,
        };
        app.insert_resource(crate::intro::MatchIntro::showing(part));
        app.update();
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(
            app.world().resource::<Steps>().0,
            1,
            "steps during an intro"
        );
        app.world_mut().resource_mut::<Paused>().on = true;
        app.world_mut().run_schedule(FixedUpdate);
        assert_eq!(app.world().resource::<Steps>().0, 1, "holds while paused");
    }

    fn school_app(lesson: usize) -> App {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let hoops = std::array::from_fn(|side| street_sim::Hoop {
            rim: [if side == 0 { 37.0 } else { -37.0 }, 11.75, 0.0],
            rim_radius: 0.75,
        });
        let players = (0..6)
            .map(|index| {
                street_sim::Player::new(
                    index / 3,
                    [
                        if index < 3 { 20.0 } else { -20.0 },
                        (index % 3) as f32 * 8.0,
                    ],
                    0.0,
                )
            })
            .collect();
        let mut world = SimWorld::new(hoops, players, street_sim::Rules::default(), 1);
        world.phase = Phase::Live;
        world.possession = if matches!(lesson, 7..=11 | 16) { 1 } else { 0 };
        // Live play in the game's own phase too (a pass launched in its
        // restart is the inbound's, which no lesson counts).
        world.game_phase.phase = street_sim::game_phase::phase::FRONT_COURT;
        world.game_phase.possession = world.possession as i32;
        world.ball.hold(if world.possession == 0 { 0 } else { 3 });
        let setup = setup::MatchSetup {
            teams: [0, 1],
            lineups: [[0, 1, 2], [3, 4, 5]],
            court: "RUCKER".into(),
            rules: street_sim::Rules::default(),
            seed: 1,
            demo: false,
            watch: false,
            seats: vec![(Device::Keyboard, 0)],
            users: vec![Some("A".into())],
            online: false,
        };
        let mut progress = street_sim::story::Story::default();
        progress.records.add_user("A").unwrap();
        school::begin_for_setup(&mut progress, &setup);
        assert!(school::start_for_setup(&mut progress, &setup, lesson));
        let path = std::env::temp_dir().join(format!(
            "street-school-callback-{}-{}.dat",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let control = controls::Control::new(&setup.seats, false, &world);
        let mut options = crate::options::Options::default();
        options.0.replays = 2; // Do not automatically replay a test basket.
        let mut app = App::new();
        app.insert_resource(Sim {
            world,
            inputs: vec![Input::default(); 6],
            call: String::new(),
            pass_style: vec![PassStyle::Normal; 6],
        })
        .insert_resource(Stats::new(6))
        .init_resource::<Replay>()
        .init_resource::<TickEvents>()
        .init_resource::<LessonPassed>()
        .init_resource::<school::LessonRun>()
        .insert_resource(story::Running(Some(story::Run::School(lesson))))
        .insert_resource(story::Save {
            path,
            story: progress,
            last_error: None,
        })
        .insert_resource(control)
        .insert_resource(setup)
        .insert_resource(options)
        .insert_resource(PlayerNames(
            (0..6).map(|index| format!("P{index}")).collect(),
        ))
        .insert_resource(TeamNames(
            ["Home".into(), "Away".into()],
            [String::new(), String::new()],
        ))
        .add_systems(FixedUpdate, step_sim);
        app
    }

    /// Events a test posts into the update's `TickEvents` after the
    /// simulation's step, as the simulation would have.
    #[derive(Resource, Default)]
    struct Posted(Vec<street_sim::Event>);

    /// A restart a test puts in the world after the simulation's step, as
    /// a basket's update would have left it.
    #[derive(Resource, Default)]
    struct PostedRestart(Option<(street_sim::Ball, Phase)>);

    /// A plain match stepping as `GamePlugin` steps it: the simulation, then
    /// the replay manager and the gamebreaker task.
    fn match_app() -> App {
        let mut app = school_app(1);
        app.insert_resource(story::Running(None))
            .init_resource::<crate::tape::Tape>()
            .init_resource::<crate::tape::GamebreakerTask>()
            .init_resource::<crate::tape::Director>()
            .init_resource::<Posted>()
            .init_resource::<PostedRestart>()
            .add_systems(
                FixedUpdate,
                (
                    (|mut posted: ResMut<Posted>,
                      mut restart: ResMut<PostedRestart>,
                      mut sim: ResMut<Sim>,
                      mut tick: ResMut<TickEvents>| {
                        tick.events.append(&mut posted.0);
                        if let Some((ball, phase)) = restart.0.take() {
                            sim.world.ball = ball;
                            sim.world.phase = phase;
                        }
                    })
                    .after(step_sim),
                    crate::tape::step.after(step_sim),
                )
                    .chain(),
            );
        app
    }

    /// The crowd's calls come off the match's own events after the
    /// simulation's step: a basket cheers (twice for a dunk), a trick gasps,
    /// each reaching the share `FUN_001a75a8` computes from the lead; a
    /// replay's re-stepped ticks call nothing.
    #[test]
    fn the_crowd_reacts_to_the_matchs_events_like_fun_001a75a8() {
        use street_render::crowd::{CrowdReactions, Reaction};
        let mut app = match_app();
        app.init_resource::<CrowdReactions>()
            .add_systems(FixedUpdate, crowd_reactions.after(crate::tape::step));
        school_steps(&mut app, 1);
        assert!(app.world().resource::<CrowdReactions>().0.is_empty());
        app.world_mut().resource_mut::<Sim>().world.score = [14, 7];
        app.world_mut().resource_mut::<Posted>().0 = vec![
            street_sim::Event::Score {
                team: 0,
                player: 0,
                points: 2,
                basket: Basket::Dunk { advanced: false },
            },
            street_sim::Event::Trick {
                player: 3,
                level: 1,
                script: 0,
                fakeout: None,
                points: 500,
            },
        ];
        school_steps(&mut app, 1);
        let to_win = match app.world().resource::<Sim>().world.rules.goal {
            Goal::Points(n) => n,
            other => panic!("{other:?}"),
        };
        let lead = app.world().resource::<Sim>().world.score[0];
        let fraction = (lead as f32 / to_win as f32 * 0.5 + 0.5).min(1.0);
        let cheer = Reaction {
            fraction,
            cheer: true,
        };
        let gasp = Reaction {
            fraction,
            cheer: false,
        };
        assert_eq!(
            std::mem::take(&mut app.world_mut().resource_mut::<CrowdReactions>().0),
            [cheer, cheer, gasp]
        );
        // The crowd's tick takes them.
        app.add_systems(
            FixedUpdate,
            street_render::crowd::step.after(crowd_reactions),
        );
        app.world_mut().resource_mut::<Posted>().0 = vec![street_sim::Event::Pickup { player: 0 }];
        school_steps(&mut app, 1);
        assert!(app.world().resource::<CrowdReactions>().0.is_empty());
    }

    #[test]
    fn a_gamebreaker_holds_the_game_for_its_two_slowed_replays_like_fun_00144ef8() {
        let mut app = match_app();
        school_steps(&mut app, 120);
        let tick = |app: &App| app.world().resource::<Sim>().world.tick;
        let live = tick(&app);
        assert_eq!(live, 120);
        assert_eq!(app.world().resource::<crate::tape::Tape>().recorded(), 120);
        // The shot goes up (slot 0x204): the task starts.
        app.world_mut().resource_mut::<Posted>().0 = vec![street_sim::Event::Gamebreaker {
            player: 0,
            level: 1,
        }];
        school_steps(&mut app, 1);
        assert!(
            app.world()
                .resource::<crate::tape::GamebreakerTask>()
                .busy()
        );
        // Its wait: 60 live updates (the last asks for the first replay).
        school_steps(&mut app, 60);
        assert_eq!(tick(&app), live + 61);
        // The manager's next update records the live frame and shows the
        // span's first step: 0.8125 into the frame 89 before the newest.
        school_steps(&mut app, 1);
        let newest = live + 62;
        assert_eq!(tick(&app), newest - 89);
        let mut shown = vec![tick(&app)];
        loop {
            school_steps(&mut app, 1);
            let tape = app.world().resource::<crate::tape::Tape>();
            if !tape.replaying() {
                break;
            }
            if tape.shown().is_some() {
                shown.push(tick(&app));
            }
        }
        // 109 frames shown, then the live game back as it was.
        assert!(app.world().resource::<crate::tape::Tape>().ended);
        assert_eq!(tick(&app), newest);
        assert!(shown.len() >= 109 && shown.windows(2).all(|w| w[1] >= w[0]));
        assert!(
            app.world()
                .resource::<crate::tape::GamebreakerTask>()
                .busy()
        );
        // The second, slower replay follows; only after it does the task
        // post its end, and the game moves on.
        let mut updates = 0;
        while !app.world().resource::<crate::tape::GamebreakerTask>().ended {
            school_steps(&mut app, 1);
            updates += 1;
            assert!(updates < 400);
        }
        assert_eq!(updates, 166 + 2);
        let resumed = tick(&app);
        assert_eq!(resumed, newest + 1);
        school_steps(&mut app, 1);
        assert_eq!(tick(&app), resumed + 1);
        assert!(
            !app.world()
                .resource::<crate::tape::GamebreakerTask>()
                .busy()
        );
    }

    fn post(app: &mut App, events: Vec<street_sim::Event>) {
        app.world_mut().resource_mut::<Posted>().0 = events;
        school_steps(app, 1);
    }

    fn replays(app: &mut App, option: u8) {
        app.world_mut()
            .resource_mut::<crate::options::Options>()
            .0
            .replays = option;
    }

    /// A dunk's basket on Always: the director (FUN_003248d8) asks for the
    /// span from 90 updates before the release, 54 updates later
    /// (FUN_00324cc8); the manager then holds the game and shows it at
    /// 0.90625, the prompt coming up 54 updates in; at its end the live
    /// game is back and the pending restart snaps both teams to their
    /// spots (FUN_00142648, FUN_001407c8).
    #[test]
    fn an_instant_replay_ending_on_a_restart_snaps_both_teams_like_fun_00142648() {
        let mut app = match_app();
        replays(&mut app, 0);
        school_steps(&mut app, 120);
        let tick = |app: &App| app.world().resource::<Sim>().world.tick;
        // The release (slots 0x64 and 0x5c) at tick 121, the basket at 131.
        post(
            &mut app,
            vec![street_sim::Event::Slam { player: 0, hoop: 0 }],
        );
        assert_eq!(tick(&app), 121);
        school_steps(&mut app, 9);
        // The simulation's draw for the director follows the basket
        // (Event::ReplayRoll): Always plays whatever it draws.
        post(
            &mut app,
            vec![
                street_sim::Event::Score {
                    team: 0,
                    player: 0,
                    points: 2,
                    basket: Basket::Dunk { advanced: false },
                },
                street_sim::Event::ReplayRoll { draw: 99 },
            ],
        );
        assert_eq!(tick(&app), 131);
        let tape = app.world().resource::<crate::tape::Tape>();
        assert!(tape.holding() && !tape.busy() && tape.offered());
        assert_eq!(
            app.world().resource::<crate::tape::Director>().kind,
            Some(1)
        );
        // The game runs on through the delay.
        school_steps(&mut app, 52);
        assert!(!app.world().resource::<crate::tape::Tape>().busy());
        assert_eq!(tick(&app), 183);
        // The restart under way when it starts: team 1 inbounds, under the
        // basket team 0 scored on (+x).
        app.world_mut().resource_mut::<PostedRestart>().0 = Some((
            street_sim::Ball::loose_at([36.0, 2.0, 0.0]),
            Phase::DeadBall { left: 1.0, team: 1 },
        ));
        school_steps(&mut app, 1);
        let live = 184;
        let tape = app.world().resource::<crate::tape::Tape>();
        assert!(tape.busy() && tape.started);
        assert_eq!(tape.mode, crate::tape::mode::SHOT);
        assert_eq!(tape.speed(), crate::tape::INSTANT_SPEED);
        // 100 frames (131 - 121 + 90) and the delay's 53: the first shown
        // is 0.90625 into the frame 152 before the newest.
        assert_eq!(tick(&app), live - 152);
        assert_eq!(tape.live().map(|w| w.tick), Some(live));
        assert!(matches!(
            tape.live().map(|w| w.phase),
            Some(Phase::DeadBall { team: 1, .. })
        ));
        // What the end owes: the held game, its restart snapped.
        let mut snapped = tape.live().unwrap().clone();
        snapped.snap_formation(1);
        assert!(app.world().resource::<Replay>().playing.is_some());
        let mut shown = 1;
        let mut prompt_at = None;
        loop {
            school_steps(&mut app, 1);
            let tape = app.world().resource::<crate::tape::Tape>();
            if !tape.busy() {
                break;
            }
            assert!(app.world().resource::<TickEvents>().replay);
            assert!(app.world().resource::<TickEvents>().events.is_empty());
            shown += 1;
            if tape.prompt.is_some() && prompt_at.is_none() {
                prompt_at = Some(shown);
            }
        }
        // 152 / 0.90625 rounds up to 168 updates: 167 steps and the one
        // asking to stop; the prompt on the 54th step.
        assert_eq!(shown, 168);
        assert_eq!(prompt_at, Some(54));
        let tape = app.world().resource::<crate::tape::Tape>();
        assert!(tape.ended && tape.prompt.is_none() && !tape.offered());
        assert!(app.world().resource::<Replay>().playing.is_none());
        assert_eq!(app.world().resource::<crate::tape::Director>().kind, None);
        let w = &app.world().resource::<Sim>().world;
        assert_eq!(w.tick, live);
        // FUN_001407c8: the restarting team's inbounder has the ball at his
        // spot and both teams stand at the strategies' spots
        // (street_sim::restart's own tests check the spots); the restart
        // goes on until all six are ready and he passes in.
        let inbounder = match w.ball.state {
            street_sim::BallState::Held(i) => i,
            other => panic!("{other:?}"),
        };
        assert_eq!(w.team_of(inbounder), 1);
        assert!(matches!(w.phase, Phase::DeadBall { team: 1, .. }));
        assert_eq!(w.ball.state, snapped.ball.state);
        for (p, q) in w.players.iter().zip(&snapped.players) {
            assert_eq!((p.position, p.heading), (q.position, q.heading));
        }
        // The game goes on.
        school_steps(&mut app, 1);
        assert_eq!(tick(&app), live + 1);
    }

    /// Manual: a basket offers the replay (FUN_00329af0): the prompt shows,
    /// the manager records 54 more frames and stops while the game goes on.
    /// The trick button replays it at 0.8125 (FUN_00329c68), the prompt
    /// back 60 updates later; the pass button stops it (FUN_00130b88 to
    /// FUN_00329d38). The restart's pass withdraws an offer (FUN_00324850).
    #[test]
    fn manual_replays_wait_for_the_trick_button_like_fun_00329c68() {
        let mut app = match_app();
        let tick = |app: &App| app.world().resource::<Sim>().world.tick;
        let tape = |app: &App| {
            let t = app.world().resource::<crate::tape::Tape>();
            (t.busy(), t.offered(), t.prompt, t.recorded())
        };
        let press = |app: &mut App, buttons: u16| {
            app.world_mut().resource_mut::<controls::Control>().seats[0]
                .raw
                .buttons = buttons;
        };
        let jumper = street_sim::Event::Score {
            team: 0,
            player: 0,
            points: 1,
            basket: Basket::Jumper { long: false },
        };
        school_steps(&mut app, 120);
        post(&mut app, vec![jumper]);
        assert_eq!(tape(&app), (false, true, Some(0), 121));
        school_steps(&mut app, 60);
        // 54 frames recorded from the basket's update, then none; the game
        // ran on.
        assert_eq!(tape(&app), (false, true, Some(0), 174));
        assert_eq!(tick(&app), 181);
        press(&mut app, street_sim::button::TRICK);
        school_steps(&mut app, 1);
        let t = app.world().resource::<crate::tape::Tape>();
        assert!(t.busy() && t.started && t.prompt.is_none());
        assert_eq!(t.speed(), crate::tape::REPEAT_SPEED);
        assert_eq!(t.live().map(|w| w.tick), Some(182));
        // The offer's span is every frame recorded when the basket went in
        // (120, clamped from 121 + 90): 0.8125 into the frame 119 before
        // the newest recorded one (tick 174, the basket's update and 53
        // more).
        assert_eq!(tick(&app), 174 - 118);
        // Held, the button does nothing more; the prompt returns on the
        // 60th step (the press's update counted).
        for _ in 0..58 {
            school_steps(&mut app, 1);
            assert!(app.world().resource::<crate::tape::Tape>().prompt.is_none());
        }
        school_steps(&mut app, 1);
        assert_eq!(app.world().resource::<crate::tape::Tape>().prompt, Some(0));
        // The cancel button: the live game back, the offer over.
        press(&mut app, street_sim::button::PASS);
        school_steps(&mut app, 1);
        let t = app.world().resource::<crate::tape::Tape>();
        assert!(t.ended && !t.busy() && !t.offered() && t.prompt.is_none());
        assert_eq!(tick(&app), 182);
        press(&mut app, 0);
        school_steps(&mut app, 1);
        assert_eq!(tick(&app), 183);
        // A new offer; the restart's pass takes it away and recording goes on.
        post(&mut app, vec![jumper]);
        assert!(tape(&app).1);
        post(&mut app, vec![street_sim::Event::Inbound { player: 3 }]);
        let (busy, offered, prompt, recorded) = tape(&app);
        assert!(!busy && !offered && prompt.is_none());
        school_steps(&mut app, 1);
        assert_eq!(tape(&app).3, (recorded + 1).min(crate::tape::FRAMES));
    }

    /// No instant replay while a gamebreaker task runs (raw 0x324d0c, and
    /// 0x329cac for the button), nor with a side's meter full; the won
    /// game's replay is forced, after 144 updates, with no prompt.
    #[test]
    fn instant_replays_keep_out_of_gamebreakers_and_full_meters() {
        let score = street_sim::Event::Score {
            team: 0,
            player: 0,
            points: 2,
            basket: Basket::Dunk { advanced: false },
        };
        let slam = street_sim::Event::Slam { player: 0, hoop: 0 };
        // The simulation draws for the director at a basket that does not
        // win the game (Event::ReplayRoll).
        let roll = street_sim::Event::ReplayRoll { draw: 0 };
        let holding = |app: &App| app.world().resource::<crate::tape::Tape>().holding();
        // A gamebreaker's task.
        let mut app = match_app();
        replays(&mut app, 0);
        school_steps(&mut app, 120);
        post(
            &mut app,
            vec![street_sim::Event::Gamebreaker {
                player: 0,
                level: 1,
            }],
        );
        post(&mut app, vec![slam]);
        post(&mut app, vec![score, roll]);
        assert!(!holding(&app));
        assert_eq!(app.world().resource::<crate::tape::Director>().kind, None);
        // A full meter.
        let mut app = match_app();
        replays(&mut app, 0);
        school_steps(&mut app, 120);
        app.world_mut().resource_mut::<Sim>().world.teams[1].meter = street_sim::street::METER_FULL;
        post(&mut app, vec![slam]);
        post(&mut app, vec![score, roll]);
        assert!(!holding(&app));
        // The won game: forced, even with the meter full and on Manual.
        replays(&mut app, 2);
        post(&mut app, vec![slam]);
        app.world_mut().resource_mut::<Sim>().world.phase = Phase::Over { winner: 0 };
        post(&mut app, vec![score]);
        let t = app.world().resource::<crate::tape::Tape>();
        assert!(t.holding() && !t.offered() && t.prompt.is_none());
        school_steps(&mut app, crate::tape::WON_DELAY as usize - 2);
        assert!(!app.world().resource::<crate::tape::Tape>().busy());
        school_steps(&mut app, 1);
        assert!(app.world().resource::<crate::tape::Tape>().busy());
    }

    fn school_steps(app: &mut App, count: usize) {
        for _ in 0..count {
            app.world_mut().run_schedule(FixedUpdate);
        }
    }

    /// The result's commit: `FUN_0018eec0` runs in the game's update
    /// before the actors and the ball, so a result their posts set (or the
    /// shot clock's, in the game's update before it) commits 54 updates
    /// after the post, 53 steps after the step that posted it; the
    /// manager's own (the expected side losing the ball, Switch Players)
    /// counts from the next update ([`street_sim::school::Run::advance`]).
    fn school_delay(app: &mut App, lesson: usize, passed: bool) {
        let delay = street_sim::school::delay(lesson) as usize;
        school_commit(app, lesson, passed, delay - 1);
    }

    /// The commit `steps` steps from now.
    fn school_commit(app: &mut App, lesson: usize, passed: bool, steps: usize) {
        school_steps(app, steps - 1);
        assert_eq!(app.world().resource::<LessonPassed>().0, None);
        school_steps(app, 1);
        let expected = Some(if passed { Ok(lesson) } else { Err(()) });
        assert_eq!(app.world().resource::<LessonPassed>().0, expected);
        assert!(matches!(
            app.world().resource::<Sim>().world.phase,
            Phase::Over { .. }
        ));
        // The commit keeps its change in the game data: the card's save
        // (over the load back) writes the file, not the commit.
        let path = app.world().resource::<story::Save>().path.clone();
        assert!(!path.exists());
        app.world_mut().resource_mut::<story::Save>().last_error = Some("already saved".into());
        school_steps(app, 150); // Actual fixed updates through the results delay.
        assert_eq!(app.world().resource::<LessonPassed>().0, expected);
        let save = app.world().resource::<story::Save>();
        assert_eq!(save.last_error.as_deref(), Some("already saved"));
        assert_eq!(save.story.school_profile.attempts[lesson], 1);
        assert_eq!(
            save.story.school_profile.challenges[lesson],
            if passed { 8 } else { 4 }
        );
        assert!(!path.exists());
    }

    #[test]
    fn school_fixed_steps_wait_for_a_catch_and_commit_once_without_a_deadline() {
        for lesson in [0, 17] {
            let mut app = school_app(lesson);
            app.world_mut().resource_mut::<Sim>().world.tick = 150 * 60;
            app.world_mut().resource_mut::<Sim>().inputs[0] = Input::default().with(
                street_sim::button::PASS
                    | if lesson == 17 {
                        street_sim::button::L3
                    } else {
                        0
                    },
            );
            school_steps(&mut app, 1);
            assert!(
                app.world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| matches!(event, street_sim::Event::Pass { .. }))
            );
            assert_eq!(app.world().resource::<LessonPassed>().0, None);
            assert!(!app.world().resource::<story::Save>().path.exists());
            app.world_mut()
                .resource_mut::<Sim>()
                .inputs
                .fill(Input::default());
            let mut caught = false;
            for _ in 0..120 {
                school_steps(&mut app, 1);
                if app
                    .world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| matches!(event, street_sim::Event::Catch { .. }))
                {
                    caught = true;
                    break;
                }
            }
            assert!(caught, "actual pass flight must reach a receiving callback");
            assert_eq!(app.world().resource::<LessonPassed>().0, None);
            school_delay(&mut app, lesson, true);
        }
        let mut failure = school_app(0);
        {
            let world = &mut failure.world_mut().resource_mut::<Sim>().world;
            // The clock has seen the side with the ball (`DAT_003d30c4`).
            world.clock.side = world.possession as i32;
            world.shot_clock = street_sim::TICK_SECONDS * 0.5;
        }
        school_steps(&mut failure, 1);
        assert!(
            failure
                .world()
                .resource::<TickEvents>()
                .events
                .iter()
                .any(|event| matches!(event, street_sim::Event::ShotClockViolation { .. }))
        );
        assert_eq!(failure.world().resource::<LessonPassed>().0, None);
        school_delay(&mut failure, 0, false);
    }

    /// In the School's one game a commit brings the lesson menu back on
    /// the lesson and the game plays on (`FUN_002b9988`); the menu holds
    /// the gameplay until a lesson launches.
    #[test]
    fn school_commits_reopen_the_lesson_menu_and_hold_the_game() {
        let mut app = school_app(0);
        app.init_resource::<crate::school_menu::LessonMenu>();
        {
            let world = &mut app.world_mut().resource_mut::<Sim>().world;
            world.clock.side = world.possession as i32;
            world.shot_clock = street_sim::TICK_SECONDS * 0.5;
        }
        school_steps(&mut app, 1);
        school_steps(&mut app, street_sim::school::delay(0) as usize - 1);
        assert_eq!(app.world().resource::<LessonPassed>().0, Some(Err(())));
        let menu = app.world().resource::<crate::school_menu::LessonMenu>();
        assert!(menu.open);
        assert_eq!(menu.lesson(), 0);
        assert!(!matches!(
            app.world().resource::<Sim>().world.phase,
            Phase::Over { .. }
        ));
        let tick = app.world().resource::<Sim>().world.tick;
        school_steps(&mut app, 30);
        assert_eq!(app.world().resource::<Sim>().world.tick, tick);
        assert!(!app.world().resource::<story::Save>().path.exists());
    }

    #[test]
    fn school_fixed_steps_switch_by_the_actual_controller_selection() {
        let mut app = school_app(7);
        app.add_systems(FixedUpdate, controls::fill_inputs.before(step_sim));
        school_steps(&mut app, 1); // Capture the native selected-actor baseline.
        assert_eq!(app.world().resource::<LessonPassed>().0, None);
        let previous = app
            .world()
            .resource::<controls::Control>()
            .first_player()
            .unwrap();
        // Cross on defence: the simulation switches in the step
        // (`street_sim::control`) and the seat follows it at the next.
        app.world_mut().resource_mut::<controls::Control>().seats[0].pad =
            Input::default().with(street_sim::button::PASS);
        school_steps(&mut app, 1);
        app.world_mut().resource_mut::<controls::Control>().seats[0].pad = Input::default();
        school_steps(&mut app, 1);
        assert_ne!(
            app.world().resource::<controls::Control>().first_player(),
            Some(previous)
        );
        assert_eq!(app.world().resource::<LessonPassed>().0, None);
        // The manager sees a switch in the step that makes it, unless that
        // update's manager posted the prompt (Switch Players' check passes
        // as soon as the lesson is out of its restart, here the first
        // step): its post returns before the check at its end
        // (`0x18f22c`), so the next update's manager counts it.
        school_commit(&mut app, 7, true, street_sim::school::delay(7) as usize - 1);
    }

    #[test]
    fn school_fixed_steps_give_and_go_needs_the_cpu_and_then_human_catches() {
        // One person on 0 and the computer on the rest, as the match plays
        // it: the right stick's pass holds the person on the passer (the
        // give 'n go, `street_sim::control`), and Cross calls the computer's
        // catcher's pass back.
        let mut app = school_app(22);
        let drive = |app: &mut App, input: Input| {
            let mut sim = app.world_mut().resource_mut::<Sim>();
            sim.inputs.fill(Input::computer());
            sim.inputs[0] = input;
        };
        let mut input = Input::default().with(street_sim::button::PASS);
        input.set_aim([0.0, 1.0]);
        drive(&mut app, input);
        school_steps(&mut app, 1);
        drive(&mut app, Input::default());
        let mut first_caught = false;
        for _ in 0..120 {
            school_steps(&mut app, 1);
            if app
                .world()
                .resource::<TickEvents>()
                .events
                .iter()
                .any(|event| matches!(event, street_sim::Event::Catch { player } if *player != 0))
            {
                first_caught = true;
                break;
            }
        }
        assert!(first_caught);
        school_steps(&mut app, 54);
        assert_eq!(app.world().resource::<LessonPassed>().0, None);
        drive(&mut app, Input::default().with(street_sim::button::PASS));
        school_steps(&mut app, 1);
        drive(&mut app, Input::default());
        let mut returned = false;
        for _ in 0..120 {
            school_steps(&mut app, 1);
            if app
                .world()
                .resource::<TickEvents>()
                .events
                .contains(&street_sim::Event::Catch { player: 0 })
            {
                returned = true;
                break;
            }
        }
        assert!(returned);
        school_delay(&mut app, 22, true);
    }

    #[test]
    fn school_fixed_steps_require_a_made_gamebreaker_and_fail_its_miss() {
        for made in [false, true] {
            let mut app = school_app(14);
            app.world_mut().resource_mut::<Sim>().world.teams[0].meter =
                street_sim::street::METER_FULL;
            app.world_mut().resource_mut::<Sim>().inputs[0] = Input::default()
                .with(street_sim::button::SHOOT | street_sim::button::L1 | street_sim::button::R1);
            school_steps(&mut app, 1);
            assert!(
                app.world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| matches!(event, street_sim::Event::Gamebreaker { .. }))
            );
            assert_eq!(app.world().resource::<LessonPassed>().0, None);
            app.world_mut()
                .resource_mut::<Sim>()
                .inputs
                .fill(Input::default());
            let mut finished = false;
            for _ in 0..240 {
                if let Some(shot) = app
                    .world_mut()
                    .resource_mut::<Sim>()
                    .world
                    .ball
                    .shot
                    .as_mut()
                {
                    shot.made = made;
                }
                school_steps(&mut app, 1);
                if app
                    .world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| {
                        matches!(
                            event,
                            street_sim::Event::Score { .. } | street_sim::Event::RimOut
                        )
                    })
                {
                    finished = true;
                    break;
                }
                assert_eq!(app.world().resource::<LessonPassed>().0, None);
            }
            assert!(finished);
            school_delay(&mut app, 14, made);
        }
    }

    #[test]
    fn school_fixed_steps_pocket_cancel_and_later_failures_replace_pending_results() {
        for (lesson, cancel, expected) in [(23, false, true), (14, false, false), (24, true, true)]
        {
            let mut app = school_app(lesson);
            {
                let mut sim = app.world_mut().resource_mut::<Sim>();
                sim.world.teams[0].meter = street_sim::street::METER_FULL;
                sim.world.teams[1].pocketed = cancel;
                sim.inputs[0] = Input::default().with(
                    street_sim::button::SELECT | if cancel { street_sim::button::L1 } else { 0 },
                );
            }
            school_steps(&mut app, 1);
            assert!(
                app.world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| matches!(
                        event,
                        street_sim::Event::Pocket { .. } | street_sim::Event::Cancel { .. }
                    ))
            );
            assert_eq!(app.world().resource::<LessonPassed>().0, None);
            app.world_mut()
                .resource_mut::<Sim>()
                .inputs
                .fill(Input::default());
            school_delay(&mut app, lesson, expected);
        }
        // A later native callback can replace a pending success before commit.
        let mut app = school_app(23);
        app.world_mut().resource_mut::<Sim>().world.teams[0].meter = street_sim::street::METER_FULL;
        app.world_mut().resource_mut::<Sim>().inputs[0] =
            Input::default().with(street_sim::button::SELECT);
        school_steps(&mut app, 1);
        app.world_mut()
            .resource_mut::<Sim>()
            .inputs
            .fill(Input::default());
        school_steps(&mut app, 10);
        app.world_mut().resource_mut::<Sim>().world.shot_clock = street_sim::TICK_SECONDS * 0.5;
        school_steps(&mut app, 1);
        assert!(
            app.world()
                .resource::<TickEvents>()
                .events
                .iter()
                .any(|event| matches!(event, street_sim::Event::ShotClockViolation { .. }))
        );
        school_delay(&mut app, 23, false);
    }

    #[test]
    fn school_fixed_steps_mapped_entry_seeds_the_enemy_pocket_for_cancel() {
        for lesson in [14, 23, 24, 25] {
            let mut app = school_app(lesson);
            // This is the same entry helper used after production load_match.
            school::stage_world(
                &mut app.world_mut().resource_mut::<Sim>().world,
                lesson,
                0,
                0,
                &[],
            );
            let world = &app.world().resource::<Sim>().world;
            assert_eq!(world.possession, 0);
            assert_eq!(world.teams[0].meter, street_sim::street::METER_FULL);
            assert_eq!(world.teams[0].pocketed, lesson == 25);
            assert_eq!(world.teams[1].pocketed, lesson == 24);
            if lesson != 24 {
                continue;
            }
            // The launch is a restart at the spots (`FUN_0018ec28`): step
            // its inbound play to its end, then the real Cancel control by
            // whoever of the side holds the ball, and its 90-update result.
            let mut holder = None;
            for _ in 0..400 {
                let world = &app.world().resource::<Sim>().world;
                holder = world.handler().filter(|&h| world.team_of(h) == 0);
                if world.phase == Phase::Live && holder.is_some() {
                    break;
                }
                school_steps(&mut app, 1);
            }
            assert_eq!(app.world().resource::<Sim>().world.phase, Phase::Live);
            let holder = holder.expect("the side's inbound reached a teammate");
            app.world_mut().resource_mut::<Sim>().inputs[holder] =
                Input::default().with(street_sim::button::SELECT | street_sim::button::L1);
            school_steps(&mut app, 1);
            assert!(
                app.world()
                    .resource::<TickEvents>()
                    .events
                    .contains(&street_sim::Event::Cancel { team: 0 })
            );
            app.world_mut()
                .resource_mut::<Sim>()
                .inputs
                .fill(Input::default());
            school_delay(&mut app, 24, true);
        }
    }

    #[test]
    fn school_fixed_steps_short_and_long_baskets_use_range_not_the_display_score() {
        for (lesson, long, pass) in [
            (1, false, true),
            (1, true, false),
            (2, true, true),
            (2, false, false),
        ] {
            let mut app = school_app(lesson);
            {
                let mut sim = app.world_mut().resource_mut::<Sim>();
                // Over the ring's middle, just above the rim's plane and
                // dropping: the basket is the ring's (`FUN_001515a8`).
                let rim = sim.world.hoops[0].rim;
                sim.world.ball.state = street_sim::BallState::Shot;
                sim.world.ball.position = [rim[0], street_sim::NATIVE_RIM_HEIGHT + 0.05, rim[2]];
                sim.world.ball.velocity = [0.0, -6.0, 0.0];
                sim.world.ball.shot = Some(street_sim::ball::Shot {
                    shooter: 0,
                    hoop: 0,
                    made: true,
                    points: if long { 2 } else { 1 },
                    gamebreaker: 0,
                    layup: false,
                    tried: 0,
                    slam: None,
                });
            }
            school_steps(&mut app, 1); // Finish an actual World shot, not an injected Event.
            assert!(app.world().resource::<TickEvents>().events.iter().any(|event|
                matches!(event, street_sim::Event::Score { points, .. } if *points == if long { 2 } else { 1 })));
            assert_eq!(app.world().resource::<LessonPassed>().0, None);
            // A wrong basket changes expected possession and fails in
            // 18eec0 (the manager's own result).
            let delay = street_sim::school::delay(lesson) as usize;
            school_commit(&mut app, lesson, pass, if pass { delay - 1 } else { delay });
        }
    }

    #[test]
    fn school_fixed_steps_use_the_disc_trick_identity_instead_of_its_value_class() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let clips = setup::load_clips(&mut disc.iso).unwrap();
        for script in [5, 20, 24, 21, 69] {
            let mut equipped: [Vec<u8>; 3] = Default::default();
            equipped[2] = vec![street_sim::story::EMPTY_MOVE_SLOT; 11];
            if let Some(row) = disc
                .moves
                .iter()
                .find(|row| row.kind == 2 && clips.scripts.row_script(row.index) == Some(script))
            {
                equipped[2][0] = row.index as u8;
            }
            let moves = clip::PlayerMoves(vec![(0, equipped)]);
            let native = clip::resolved_trick_slots(&clips, &moves, &disc, 0).unwrap();
            let slot = native
                .slots
                .iter()
                .position(|identity| identity.script == script)
                .expect("the original disc must resolve the tested native identity");
            let mask = street_sim::player::TRICK_SLOT_MASKS[slot];
            let buttons = [
                (0x4000, street_sim::button::L1),
                (0x8000, street_sim::button::R1),
                (0x10000, street_sim::button::L2),
                (0x20000, street_sim::button::R2),
            ]
            .into_iter()
            .filter(|(bit, _)| mask & bit != 0)
            .fold(street_sim::button::TRICK, |buttons, (_, button)| {
                buttons | button
            });
            let mut app = school_app(20);
            app.world_mut().resource_mut::<Sim>().world.players[0].native_tricks = Some(native);
            app.world_mut().resource_mut::<Sim>().world.players[0]
                .ratings
                .handles = 1.0;
            // Dribbling (a fresh take standing still clears out instead).
            app.world_mut().resource_mut::<Sim>().world.players[0].dribble = 1;
            app.world_mut().resource_mut::<Sim>().inputs[0] = Input::default().with(buttons);
            school_steps(&mut app, 1);
            app.world_mut()
                .resource_mut::<Sim>()
                .inputs
                .fill(Input::default());
            // The trick scores at its script's end.
            let mut landed = false;
            for _ in 0..=street_sim::trick::length(script) {
                school_steps(&mut app, 1);
                if app
                    .world()
                    .resource::<TickEvents>()
                    .events
                    .iter()
                    .any(|event| matches!(event, street_sim::Event::Trick { .. }))
                {
                    landed = true;
                    break;
                }
            }
            assert!(landed);
            if script == 69 {
                school_steps(&mut app, 54);
                assert_eq!(app.world().resource::<LessonPassed>().0, None);
                assert!(!app.world().resource::<story::Save>().path.exists());
            } else {
                school_delay(&mut app, 20, true);
            }
        }
    }

    fn seat(device: Option<Device>) -> Seat {
        Seat {
            number: 0,
            device,
            team: 0,
            player: 0,
            pad: Input::default(),
            raw: Input::default(),
            edges: street_sim::input::ButtonEdges::default(),
        }
    }

    #[test]
    fn only_a_controller_without_a_seat_is_a_challenger() {
        let pad = Device::Pad(Entity::from_raw_u32(7).unwrap());
        // One person on the keyboard: a pad's Start is a challenge.
        assert!(is_challenger(&[seat(Some(Device::Keyboard))], pad));
        assert!(!is_challenger(
            &[seat(Some(Device::Keyboard))],
            Device::Keyboard
        ));
        // A seat on every controller: everyone is playing already.
        assert!(!is_challenger(&[seat(None)], pad));
    }
}
