//! The front end's idle attract demo: what the title (class 99) and Main
//! (3) do when nobody touches them.
//!
//! - *The idle timer.* Each arms a front-end timer of `demo_delay` seconds
//!   when it stands (its slot `+0x16c`, which the front end calls as the
//!   transition into it ends, `FUN_002dcff8`/`FUN_002dd238`: the title's
//!   `FUN_002d3848`, Main's `FUN_0029e0d0`; `FUN_002da198` on the timers at
//!   `0x4d6450`, counted on the 100 Hz timer). `demo_delay` is the
//!   executable's 90 (`FUN_0029ed60`) until the disc's
//!   `/CSCRIPTS/AUTOEXEC.CFG` sets it: `demo_delay 120` on the retail disc
//!   ([`demo_delay`]). A change of screen drops every front-end timer
//!   (`FUN_002dd1b0`, `FUN_002da0e0`), so each screen arms afresh as it
//!   arrives; a screen covered (Main's Help overlay) pauses them
//!   (`FUN_002d7ad8`, `FUN_002d7b00`, `FUN_002da128`/`FUN_002da160`). Every
//!   message Main takes restarts its timer (`FUN_0029e700`, `FUN_002da258`);
//!   the title's handler (`FUN_002d3cf0`) leaves it running.
//! - *"Loading Demo...".* At the timer's end (`FUN_002d3d68`,
//!   `FUN_0029e8f8`) the screen's `+0xf0` is set and Main takes no more
//!   presses; the help widget (0xe) shows `main_help.txt` 6, on the title in
//!   place of "Press START button" with its stars (0x14, 0x15) hidden, and a
//!   second timer of 4 s starts.
//! - *The turns.* At its end (`FUN_002d3ed8`, `FUN_0029ec08`) the front end
//!   goes, by turns (`DAT_003fd704`, 0 at the boot), to the attract's game
//!   (mode 0, `FUN_001dba68`: the picks of `FUN_001db7f8`, [`picks`]; the
//!   load, class 0x2c) or to the title again, the front end's song stopped
//!   first (`FUN_0019fd60(.., -5, 0)`, `FUN_001a8da0`).
//! - *The game.* The attract's game plays to its end at Legendary
//!   (`FUN_001d74a8(.., 2, 0)` on the options' level, the player's kept at
//!   `+0x1c0a` until Main's build puts it back, `FUN_001de450`) on the
//!   rules Main's build leaves (`FUN_001ded20`: to 21, by 2). "Press START
//!   to exit demo" (`overlays.txt` 44) stands over it; Start on any pad
//!   ends it (`FUN_00141f18`: game `+0xf0` set to -4, the task's exit two
//!   updates on, `FUN_00143180`), and so does its last basket, with no
//!   results (`FUN_002f3918`: mode 0 exits the task). The load back (class
//!   0x2f) then goes back to the title (`FUN_0029d920`, mode 0).

use bevy::prelude::*;

/// `demo_delay` as the executable sets it (`FUN_0029ed60`: `DAT_003fa5f0`
/// = 90), before the disc's `AUTOEXEC.CFG` runs.
pub const DEMO_DELAY_DEFAULT: f64 = 90.0;

/// The script the boot runs (`demo_delay 120` on the retail disc).
pub const AUTOEXEC: &str = "/CSCRIPTS/AUTOEXEC.CFG";

/// How long "Loading Demo..." stands before the turn is taken
/// (`FUN_002da198(.., 4, 1)` in `FUN_002d3d68` and `FUN_0029e8f8`).
pub const LOADING_DEMO_SECONDS: f64 = 4.0;

/// The title's and Main's classes.
pub const TITLE: u16 = 0x63;
pub const MAIN: u16 = 0x03;

/// `demo_delay` from the boot's script: its last `demo_delay <seconds>`
/// line (the console's variables take a line's name and value, `//`
/// starting a comment).
pub fn demo_delay(script: &str) -> Option<f64> {
    script
        .lines()
        .filter_map(|line| {
            let line = line.split("//").next()?.trim();
            let mut words = line.split_whitespace();
            (words.next()? == "demo_delay")
                .then(|| words.next()?.parse::<f64>().ok())
                .flatten()
        })
        .next_back()
}

/// Where the turns stand (`DAT_003fd704`): the attract's game next, or the
/// title again.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Turn {
    #[default]
    Demo,
    Title,
}

impl Turn {
    fn next(self) -> Self {
        match self {
            Turn::Demo => Turn::Title,
            Turn::Title => Turn::Demo,
        }
    }
}

/// What a step of the timers did.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Wait,
    /// The idle timer ran out: "Loading Demo..." on the screen.
    Loading,
    /// The 4 s are up: the turn to take.
    Go(Turn),
}

/// A front-end timer (`0x4d6450 + 0xa24 + i * 0x20`): its start, the
/// seconds it had run when paused (`+0xa38`), and whether it stands paused.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Timer {
    screen: u16,
    start: f64,
    before: f64,
    paused_at: Option<f64>,
}

impl Timer {
    fn new(screen: u16, now: f64) -> Self {
        Self {
            screen,
            start: now,
            before: 0.0,
            paused_at: None,
        }
    }

    /// Seconds run (`FUN_002da300`: the time since the start, or since the
    /// pause ended, plus what ran before it).
    fn run(&self, now: f64) -> f64 {
        match self.paused_at {
            Some(_) => self.before,
            None => now - self.start + self.before,
        }
    }

    fn pause(&mut self, now: f64) {
        if self.paused_at.is_none() {
            self.before = self.run(now);
            self.paused_at = Some(now);
        }
    }

    fn resume(&mut self, now: f64) {
        if self.paused_at.take().is_some() {
            self.start = now;
        }
    }
}

/// The idle demo's timers and turns.
#[derive(Resource, Clone, Debug)]
pub struct Idle {
    /// `demo_delay`, seconds.
    pub delay: f64,
    /// The idle timer of the screen on show (`+0xf8` on the title, `+0xf4`
    /// on Main).
    timer: Option<Timer>,
    /// "Loading Demo..." and its 4 s timer (`+0xfa`, `+0xf6`).
    loading: Option<Timer>,
    /// `DAT_003fd704`.
    pub turn: Turn,
    /// The last attract's court (`+0x1c08`, none at the boot), which the
    /// next draw passes over.
    pub last_court: Option<usize>,
}

impl Default for Idle {
    fn default() -> Self {
        Self::new(DEMO_DELAY_DEFAULT)
    }
}

impl Idle {
    pub fn new(delay: f64) -> Self {
        Self {
            delay,
            timer: None,
            loading: None,
            turn: Turn::Demo,
            last_court: None,
        }
    }

    /// `screen` stands (its slot `+0x16c`): its idle timer starts unless it
    /// runs already, or the screen is loading the demo.
    pub fn stand(&mut self, screen: u16, now: f64) {
        if self.timer.is_some_and(|t| t.screen == screen)
            || self.loading.is_some_and(|t| t.screen == screen)
        {
            return;
        }
        self.timer = Some(Timer::new(screen, now));
        self.loading = None;
    }

    /// The screen is gone (a change of screen drops the front end's timers).
    pub fn leave(&mut self) {
        self.timer = None;
        self.loading = None;
    }

    /// A message the screen took: Main restarts its idle timer
    /// (`FUN_002da258`); the title does not.
    pub fn press(&mut self, now: f64) {
        if let Some(t) = self.timer.as_mut()
            && t.screen == MAIN
        {
            *t = Timer::new(MAIN, now);
        }
    }

    /// The screen covered (`covered`) or uncovered: its timers pause or run
    /// on.
    pub fn cover(&mut self, covered: bool, now: f64) {
        for t in [self.timer.as_mut(), self.loading.as_mut()]
            .into_iter()
            .flatten()
        {
            if covered {
                t.pause(now);
            } else {
                t.resume(now);
            }
        }
    }

    /// Whether `screen` shows "Loading Demo..." (and Main takes no press).
    pub fn loading_demo(&self, screen: u16) -> bool {
        self.loading.is_some_and(|t| t.screen == screen)
    }

    /// The idle timer's seconds left, while one runs.
    pub fn left(&self, now: f64) -> Option<f64> {
        self.timer.map(|t| self.delay - t.run(now))
    }

    /// The timers at `now` (`FUN_002da300`, each front-end update).
    pub fn step(&mut self, now: f64) -> Step {
        if let Some(t) = self.loading
            && t.run(now) >= LOADING_DEMO_SECONDS
        {
            self.loading = None;
            let turn = self.turn;
            self.turn = turn.next();
            return Step::Go(turn);
        }
        if let Some(t) = self.timer
            && t.run(now) >= self.delay
        {
            self.timer = None;
            self.loading = Some(Timer::new(t.screen, now));
            return Step::Loading;
        }
        Step::Wait
    }
}

/// The courts the attract's draw takes from (`rand() % 10`: never the
/// eleventh, the hidden court).
pub const DRAWN_COURTS: u32 = 10;
/// The teams it draws from (`rand() % 0x1c`: the database's first 28).
pub const DRAWN_TEAMS: u32 = 28;
/// Draws past this many the rewrite stops redrawing (the original's loop
/// has no end; a fresh save opens three courts).
const REDRAWS: usize = 10_000;

/// What `FUN_001db7f8` draws from and sets: the shared generator
/// (`FUN_003a16b8`) and a side's team (`FUN_001dbb20`, which for a side no
/// person holds reseeds the generator and draws the side's three,
/// `FUN_001dbe30`).
pub trait Draws {
    fn draw(&mut self) -> u32;
    fn team_set(&mut self, side: usize, team: usize);
}

/// The attract's court and teams.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Picks {
    pub court: usize,
    pub teams: [usize; 2],
}

/// `FUN_001db7f8`: a court `rand() % 10` drawn again until it is open
/// (`FUN_001d37b0`) and not the last attract's (`+0x1c08`, none at the
/// boot); the setup's slot 0's team `rand() % 28`, set; slot 1's `rand() %
/// 28`, set, drawn and set again while it is slot 0's. Slot 0's set
/// reseeds the generator, so slot 1's draw comes from the timer's count.
/// Slot 0 plays the game's side 1 (`FUN_00185138` reads game side g's
/// players from slot g ^ 1; save state 9's attract match: slot 0's team 22
/// on side 1), so the first team drawn is side 1's: `teams` by side.
pub fn picks(d: &mut impl Draws, open: impl Fn(usize) -> bool, last: Option<usize>) -> Picks {
    let mut court = None;
    for _ in 0..REDRAWS {
        let c = (d.draw() % DRAWN_COURTS) as usize;
        if open(c) && Some(c) != last {
            court = Some(c);
            break;
        }
    }
    let court = court
        .or_else(|| (0..DRAWN_COURTS as usize).find(|&c| open(c)))
        .unwrap_or(0);
    let first = (d.draw() % DRAWN_TEAMS) as usize;
    d.team_set(1, first);
    let mut second = (d.draw() % DRAWN_TEAMS) as usize;
    d.team_set(0, second);
    for _ in 0..REDRAWS {
        if second != first {
            break;
        }
        second = (d.draw() % DRAWN_TEAMS) as usize;
        d.team_set(0, second);
    }
    Picks {
        court,
        teams: [second, first],
    }
}

/// The front end's draws on the session: the generator, and a side's team
/// set as Choose Team's commit sets it (`menu::computer_three`).
struct FrontEnd<'a> {
    session: Option<&'a mut crate::session::Session>,
    setup: &'a mut crate::setup::MatchSetup,
    disc: &'a crate::setup::Disc,
    story: &'a street_sim::story::Story,
    /// A run with no session draws from the setup's seed.
    local: street_sim::Rng,
}

impl Draws for FrontEnd<'_> {
    fn draw(&mut self) -> u32 {
        match self.session.as_deref_mut() {
            Some(s) => s.draw(),
            None => self.local.next_u32(),
        }
    }

    fn team_set(&mut self, side: usize, team: usize) {
        self.setup.teams[side] = team;
        crate::menu::computer_three(
            self.session.as_deref_mut(),
            self.setup,
            side,
            self.disc,
            self.story,
        );
    }
}

/// `FUN_001dba68(.., 0)`: the attract's game set up on `setup` (the
/// computer on both sides, the rules Main's build leaves at Legendary, the
/// picks drawn), its court kept for the next draw. The setup the menus had
/// is handed back to restore once the front end is back.
pub fn set_up(
    setup: &mut crate::setup::MatchSetup,
    session: Option<&mut crate::session::Session>,
    disc: &crate::setup::Disc,
    story: &street_sim::story::Story,
    last_court: &mut Option<usize>,
) -> crate::setup::MatchSetup {
    let before = setup.clone();
    setup.demo = true;
    setup.watch = true;
    setup.seats.clear();
    setup.users.clear();
    setup.online = false;
    setup.rules = street_sim::Rules {
        difficulty: street_sim::Difficulty::Hard,
        ..street_sim::Rules::default()
    };
    let local = street_sim::Rng::new(setup.seed);
    let mut fe = FrontEnd {
        session,
        setup,
        disc,
        story,
        local,
    };
    let picks = picks(
        &mut fe,
        |c| crate::rewards::court_unlocked(story, disc, c),
        *last_court,
    );
    *last_court = Some(picks.court);
    if let Some(c) = disc.courts.get(picks.court) {
        setup.court = c.id.clone();
    }
    before
}

/// The setup the menus had before the idle demo's game, put back when the
/// front end is back (the original's Main rebuilds its game data,
/// `FUN_001de450`; the rewrite's menus keep their picks). Set while the
/// idle demo's game runs: the front end's mode 0, whose game takes Start
/// as its end and shows no results (the command line's `--demo` keeps the
/// rewrite's pause and boards).
#[derive(Resource, Default)]
pub struct Before {
    pub setup: Option<crate::setup::MatchSetup>,
    /// The game ending (Start, or its replay over): when its last frame
    /// froze, the front end's load following [`EXIT_SECONDS`] on.
    pub leaving: Option<f64>,
}

impl Before {
    /// The idle demo's game is the one being played.
    pub fn playing(this: Option<&Self>) -> bool {
        this.is_some_and(|b| b.setup.is_some())
    }

    /// The game's end at `now`: whether its hold is over and the front
    /// end's load is due.
    pub fn leave(&mut self, now: f64) -> bool {
        now - *self.leaving.get_or_insert(now)
            >= crate::pace::hold(EXIT_SECONDS, crate::pace::DEMO_EXIT)
    }
}

/// From Start to the load back's picture (measured 0.97 s: the game gone
/// 0.22 s after the press, `FUN_00143180`'s two updates and the task's
/// exit, and the load back 0.75 s later, its last frame standing
/// meanwhile); after the won game's replay the runtime's picture came
/// 1.3 s on, within half a second.
pub const EXIT_SECONDS: f64 = 0.97;

/// What taking a turn changes.
#[derive(bevy::ecs::system::SystemParam)]
pub(crate) struct TurnParams<'w> {
    setup: ResMut<'w, crate::setup::MatchSetup>,
    disc: Res<'w, crate::setup::Disc>,
    save: Res<'w, crate::story::Save>,
    session: Option<ResMut<'w, crate::session::Session>>,
    running: ResMut<'w, crate::story::Running>,
    before: ResMut<'w, Before>,
    song: ResMut<'w, crate::audio::MenuSongCue>,
}

/// The idle demo's timers on the title and Main, each front-end update
/// before the screens take their keys: the screen standing arms its timer,
/// a press on Main restarts it, a screen covered pauses it, and the turn
/// is taken when "Loading Demo..." has stood its 4 s.
pub(crate) fn idle(
    mut idle: ResMut<Idle>,
    mut menus: ResMut<crate::menu::Menus>,
    mut t: ResMut<crate::transition::Transition>,
    input: crate::menu::MenuInput,
    time: Res<Time<Real>>,
    card: Res<crate::card::Card>,
    typing: Res<crate::typing::Typing>,
    mut turn: TurnParams,
) {
    let now = time.elapsed_secs_f64();
    // The front end's song comes back as the title's intro ends (at its
    // last frame, or at Start, before the zoom onto the billboard).
    if turn.song.0 && !t.holds_song() {
        turn.song.0 = false;
    }
    let screen = match menus.top().0 {
        crate::menu::Page::Title => Some(TITLE),
        crate::menu::Page::Main => Some(MAIN),
        _ => None,
    };
    let Some(screen) = screen.filter(|_| t.standing()) else {
        idle.leave();
        return;
    };
    idle.stand(screen, now);
    // The Help overlay (Square held) and the card's dialogs cover the
    // screen.
    idle.cover(menus.help || card.flow.is_some() || typing.busy(), now);
    if t.idle() && input.any_press() {
        idle.press(now);
    }
    match idle.step(now) {
        Step::Wait | Step::Loading => {}
        Step::Go(Turn::Demo) => {
            // `FUN_001dba68(.., 0)`, then forward to the load (class
            // 0x2c, no movie from either screen).
            let before = set_up(
                &mut turn.setup,
                turn.session.as_deref_mut(),
                &turn.disc,
                &turn.save.story,
                &mut idle.last_court,
            );
            turn.before.setup.get_or_insert(before);
            turn.before.leaving = None;
            turn.running.0 = None;
            menus.back_to_title();
            t.begin(
                crate::transition::Change::new(
                    screen,
                    0x2c,
                    crate::transition::Kind::Forward,
                    false,
                    Some(crate::Screen::Loading),
                ),
                now,
            );
        }
        Step::Go(Turn::Title) => {
            // `FUN_0019fd60(.., -5, 0)`, then forward to the title.
            turn.song.0 = true;
            menus.back_to_title();
            t.begin(
                crate::transition::Change::new(
                    screen,
                    TITLE,
                    crate::transition::Kind::Forward,
                    false,
                    None,
                ),
                now,
            );
        }
    }
}

/// Whether the screens take their keys: Main takes none while it loads the
/// demo (`FUN_0029e700` returns at once on `+0xf0`).
pub(crate) fn takes_presses(idle: Res<Idle>) -> bool {
    !idle.loading_demo(MAIN)
}

/// The front end back after the demo's game: the menus' setup as it was.
pub(crate) fn restore(mut before: ResMut<Before>, mut setup: ResMut<crate::setup::MatchSetup>) {
    before.leaving = None;
    if let Some(s) = before.setup.take() {
        *setup = s;
    }
}

/// The command line's `--demo-delay`, over the disc's.
#[derive(Resource)]
pub struct DelayOverride(pub f64);

/// `demo_delay` from the disc's boot script, the executable's when the
/// script is missing or does not set it.
pub(crate) fn read_delay(
    mut disc: ResMut<crate::setup::Disc>,
    mut idle: ResMut<Idle>,
    over: Option<Res<DelayOverride>>,
) {
    if let Some(over) = over {
        idle.delay = over.0;
        return;
    }
    let script = disc
        .iso
        .read(AUTOEXEC)
        .ok()
        .map(|b| String::from_utf8_lossy(&b).into_owned());
    idle.delay = script
        .as_deref()
        .and_then(demo_delay)
        .unwrap_or(DEMO_DELAY_DEFAULT);
}

pub struct AttractPlugin;

impl Plugin for AttractPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Idle>()
            .init_resource::<Before>()
            .add_systems(Startup, read_delay)
            .add_systems(OnEnter(crate::Screen::Menu), restore);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shared generator and the reseeds, as a test sees them.
    struct Runtime {
        rng: street_sim::Rng,
        /// The timer's count at each reseed.
        counts: Vec<u32>,
        set: Vec<(usize, usize)>,
    }

    impl Draws for Runtime {
        fn draw(&mut self) -> u32 {
            self.rng.next_u32()
        }

        fn team_set(&mut self, side: usize, team: usize) {
            self.set.push((side, team));
            if !self.counts.is_empty() {
                self.rng = street_sim::Rng::new(self.counts.remove(0));
            }
        }
    }

    /// The first idle demo of a fresh boot in the project's PCSX2 runtime
    /// (2026-10-20, `street-idle-orig/run1b`): the generator as the boot
    /// left it (nothing on the title draws from it), the turn at the
    /// timer's count 20,468. The court roll gives 2 (closed on a fresh
    /// save, drawn again), then 0; slot 0's roll team 8; slot 1's, after
    /// the reseed, team 9 — court 0, teams 8 and 9, as the game data read
    /// them: 9 plays side 0, 8 side 1.
    #[test]
    fn the_runtimes_first_demo_drew_court_0_and_teams_8_and_9() {
        let mut rt = Runtime {
            rng: street_sim::Rng::from_state([
                1_436_272_595,
                1_668_184_875,
                3_678_213_617,
                353_892_977,
                236_255_107,
                1_876_916_662,
            ]),
            counts: vec![20_468, 20_468],
            set: Vec::new(),
        };
        let open = |c: usize| matches!(c, 0 | 1 | 6);
        let p = picks(&mut rt, open, None);
        assert_eq!(
            p,
            Picks {
                court: 0,
                teams: [9, 8]
            }
        );
        assert_eq!(rt.set, [(1, 8), (0, 9)]);
        // The next demo never draws the same court.
        for seed in 0..50 {
            let mut rt = Runtime {
                rng: street_sim::Rng::new(seed),
                counts: Vec::new(),
                set: Vec::new(),
            };
            let p = picks(&mut rt, open, Some(0));
            assert!(matches!(p.court, 1 | 6), "{p:?}");
            assert_ne!(p.teams[0], p.teams[1]);
            assert!(p.teams.iter().all(|&t| t < 28));
        }
    }

    #[test]
    fn the_retail_scripts_delay_is_120_seconds() {
        let script = "// This script file is executed automatically on startup.\r\n//\r\n\r\n\
                      //media_build 1\r\n\r\n// set demo delay (in seconds)...\r\n\
                      demo_delay 120\r\n\r\nprint \"Autoexec Complete!\"\r\n";
        assert_eq!(demo_delay(script), Some(120.0));
        assert_eq!(demo_delay("// demo_delay 30\r\n"), None);
        assert_eq!(
            demo_delay("demo_delay 30\ndemo_delay 45 // later\n"),
            Some(45.0)
        );
        assert_eq!(Idle::default().delay, 90.0);
    }

    #[test]
    fn the_title_waits_out_its_delay_then_loads_the_demo_for_four_seconds() {
        let mut idle = Idle::new(120.0);
        idle.stand(TITLE, 10.0);
        // Presses on the title leave its timer running.
        idle.press(50.0);
        assert_eq!(idle.step(129.9), Step::Wait);
        assert_eq!(idle.left(129.9).map(|s| (s * 10.0).round()), Some(1.0));
        assert_eq!(idle.step(130.0), Step::Loading);
        assert!(idle.loading_demo(TITLE) && !idle.loading_demo(MAIN));
        // Standing on, the screen does not arm again while it loads.
        idle.stand(TITLE, 131.0);
        assert_eq!(idle.step(133.9), Step::Wait);
        assert_eq!(idle.step(134.0), Step::Go(Turn::Demo));
        assert!(!idle.loading_demo(TITLE));
        assert_eq!(idle.turn, Turn::Title);
    }

    #[test]
    fn main_restarts_its_timer_on_every_press_and_the_help_overlay_pauses_it() {
        let mut idle = Idle::new(120.0);
        idle.stand(MAIN, 0.0);
        idle.press(100.0);
        assert_eq!(idle.step(219.0), Step::Wait);
        // Covered for 30 s (Square held): the time does not count.
        idle.cover(true, 150.0);
        assert_eq!(idle.step(250.0), Step::Wait);
        idle.cover(false, 180.0);
        assert_eq!(idle.step(249.9), Step::Wait);
        assert_eq!(idle.step(250.0), Step::Loading);
        assert!(idle.loading_demo(MAIN));
    }

    #[test]
    fn the_turns_alternate_the_demo_and_the_title_across_both_screens() {
        let mut idle = Idle::new(120.0);
        let mut taken = Vec::new();
        let mut now = 0.0;
        for screen in [TITLE, MAIN, TITLE, TITLE] {
            idle.stand(screen, now);
            now += 120.0;
            assert_eq!(idle.step(now), Step::Loading);
            now += 4.0;
            if let Step::Go(turn) = idle.step(now) {
                taken.push(turn);
            }
            idle.leave();
        }
        assert_eq!(taken, [Turn::Demo, Turn::Title, Turn::Demo, Turn::Title]);
    }

    #[test]
    fn the_demos_end_holds_its_last_frame_before_the_load() {
        let mut demo = Before::default();
        assert!(!Before::playing(Some(&demo)) && !Before::playing(None));
        demo.setup = Some(crate::setup::MatchSetup {
            teams: [0, 1],
            lineups: [[0; 3]; 2],
            court: "RUCKER".into(),
            rules: street_sim::Rules::default(),
            seed: 2002,
            demo: false,
            watch: false,
            seats: Vec::new(),
            users: Vec::new(),
            online: false,
        });
        assert!(Before::playing(Some(&demo)));
        // Start at 10 s: the front end's load 0.97 s on.
        assert!(!demo.leave(10.0));
        assert!(!demo.leave(10.96));
        assert!(demo.leave(10.97));
    }

    #[test]
    fn a_change_of_screen_drops_the_timers() {
        let mut idle = Idle::new(120.0);
        idle.stand(MAIN, 0.0);
        idle.leave();
        assert_eq!(idle.step(500.0), Step::Wait);
        // Back on Main: a fresh 120 s from its arrival.
        idle.stand(MAIN, 500.0);
        assert_eq!(idle.step(619.0), Step::Wait);
        assert_eq!(idle.step(620.0), Step::Loading);
        // Leaving during "Loading Demo..." takes no turn.
        idle.leave();
        assert_eq!(idle.step(700.0), Step::Wait);
        assert_eq!(idle.turn, Turn::Demo);
        // Another screen's arrival replaces the timer.
        idle.stand(MAIN, 700.0);
        idle.stand(TITLE, 710.0);
        assert_eq!(idle.step(829.0), Step::Wait);
        assert_eq!(idle.step(830.0), Step::Loading);
        assert!(idle.loading_demo(TITLE));
    }
}
