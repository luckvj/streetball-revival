//! The front end, laid out like the game's own (`XLOC_FE`): a title screen;
//! the main menu (Play It, Options, ...); Play It's modes; and for a Pick Up
//! Game, Choose Sides (each controller to Away, CPU or Home), Choose Team,
//! Choose Players, Choose Court and My Rules, then Play Game.
//!
//! Keys: arrows or WASD to move, Enter, Space or J to pick, Esc or Backspace
//! to go back. Pads: the d-pad, Cross (A) to pick, Circle or Triangle (B or
//! Y) to go back.

use bevy::{app::AppExit, ecs::system::SystemParam, prelude::*};
use street_sim::{Difficulty, Mode, Rules};

use crate::{
    Screen, art,
    audio::AudioSettings,
    ballers,
    controls::{Device, MAX_SEATS},
    look::{self, Look, Style, View},
    movie, options,
    setup::{Disc, MatchSetup},
    story,
    story::CREW_SIZE,
    teams, transition,
    typing::{self, Field, Typing},
};

/// A message for the main menu (a failed load, say).
#[derive(Resource, Default)]
pub struct Message(pub String);

#[derive(Resource, Default)]
struct CheatEntries(std::collections::HashMap<Device, crate::cheats::EntryState>);

/// Menu buttons from the keyboard and every pad, pressed this frame.
#[derive(SystemParam)]
pub struct MenuInput<'w, 's> {
    keys: Res<'w, ButtonInput<KeyCode>>,
    gamepads: Query<'w, 's, (Entity, &'static Gamepad)>,
    /// Pads' sticks and D-pads, with held directions repeating.
    nav: Res<'w, crate::pads::PadNav>,
    /// The one controller the screen takes keys from, `None` for every
    /// one ([`listens_to`]).
    only: Local<'s, Option<Device>>,
}

/// Whether a screen listening to `only` takes `device`'s keys.
fn allows(only: Option<Device>, device: Device) -> bool {
    only.is_none_or(|d| d == device)
}

/// The one controller `page` takes keys from, `None` for every one:
/// Choose Region (class 0x29) takes no key but the user's pad (the game
/// data's `+0xd4`, the pad seated for the run; measured in the
/// select-questions pass, the screen took no key at all until `+0xd4`
/// named a pad). With nobody seated, any.
fn listens_to(page: Page, seats: &[(Device, usize)]) -> Option<Device> {
    match page {
        Page::League | Page::LeagueRegion(_) => seats.first().map(|&(d, _)| d),
        _ => None,
    }
}

impl MenuInput<'_, '_> {
    /// The pads this screen takes keys from.
    fn pads(&self) -> impl Iterator<Item = (Entity, &Gamepad)> {
        let only = *self.only;
        self.gamepads
            .iter()
            .filter(move |&(e, _)| allows(only, Device::Pad(e)))
    }

    /// Whether this screen takes the keyboard's keys.
    fn keyboard(&self) -> bool {
        allows(*self.only, Device::Keyboard)
    }

    /// The pads' directions this frame, of the one listened to.
    fn nav_any(&self) -> IVec2 {
        match *self.only {
            None => self.nav.any(),
            Some(Device::Pad(e)) => self.nav.of(e),
            Some(Device::Keyboard) => IVec2::ZERO,
        }
    }

    fn any(&self, keys: &[KeyCode], buttons: &[GamepadButton]) -> bool {
        (self.keyboard() && self.keys.any_just_pressed(keys.iter().copied()))
            || self
                .pads()
                .any(|(_, g)| buttons.iter().any(|&b| g.just_pressed(b)))
    }

    /// Every controller: the keyboard, then the pads as they connected.
    pub fn devices(&self) -> Vec<Device> {
        let mut pads: Vec<Entity> = self.gamepads.iter().map(|(e, _)| e).collect();
        pads.sort();
        std::iter::once(Device::Keyboard)
            .chain(pads.into_iter().map(Device::Pad))
            .collect()
    }

    /// Left (-1) or right (+1) on one controller this frame (a pad's
    /// D-pad or left stick).
    pub fn step_of(&self, device: Device) -> i32 {
        if !allows(*self.only, device) {
            return 0;
        }
        match device {
            Device::Keyboard => {
                i32::from(
                    self.keys
                        .any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD]),
                ) - i32::from(
                    self.keys
                        .any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA]),
                )
            }
            Device::Pad(e) => self.nav.of(e).x,
        }
    }

    /// Up (-1) or down (+1) on one controller this frame.
    pub fn vertical_of(&self, device: Device) -> i32 {
        if !allows(*self.only, device) {
            return 0;
        }
        match device {
            Device::Keyboard => {
                i32::from(
                    self.keys
                        .any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS]),
                ) - i32::from(
                    self.keys
                        .any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW]),
                )
            }
            Device::Pad(e) => -self.nav.of(e).y,
        }
    }

    /// Whose buttons to name in prompts: whatever was touched last.
    pub fn style(&self) -> crate::pads::ButtonStyle {
        self.nav.style
    }

    /// Confirm on one controller this frame.
    pub fn confirm_of(&self, device: Device) -> bool {
        if !allows(*self.only, device) {
            return false;
        }
        match device {
            Device::Keyboard => {
                self.keys
                    .any_just_pressed([KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ])
            }
            Device::Pad(e) => self.gamepads.get(e).is_ok_and(|(_, g)| {
                g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::Start)
            }),
        }
    }

    /// Back on one controller this frame.
    pub fn back_of(&self, device: Device) -> bool {
        if !allows(*self.only, device) {
            return false;
        }
        match device {
            Device::Keyboard => self
                .keys
                .any_just_pressed([KeyCode::Escape, KeyCode::Backspace]),
            Device::Pad(e) => self.gamepads.get(e).is_ok_and(|(_, g)| {
                g.just_pressed(GamepadButton::East) || g.just_pressed(GamepadButton::North)
            }),
        }
    }

    /// L1 (-1, Q or a pad's left shoulder) or R1 (+1, E or the right) this
    /// frame.
    pub fn shoulder(&self) -> i32 {
        i32::from(self.any(&[KeyCode::KeyE], &[GamepadButton::RightTrigger]))
            - i32::from(self.any(&[KeyCode::KeyQ], &[GamepadButton::LeftTrigger]))
    }

    /// The controller that pressed confirm this frame.
    pub fn confirmed_by(&self) -> Option<Device> {
        let keys = [KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ];
        if self.keyboard() && self.keys.any_just_pressed(keys) {
            return Some(Device::Keyboard);
        }
        self.pads()
            .find(|(_, g)| {
                g.just_pressed(GamepadButton::South) || g.just_pressed(GamepadButton::Start)
            })
            .map(|(e, _)| Device::Pad(e))
    }

    // Directions: the keys, or any pad's D-pad or left stick (held ones
    // repeat, `pads::PadNav`).
    pub fn up(&self) -> bool {
        self.any(&[KeyCode::ArrowUp, KeyCode::KeyW], &[]) || self.nav_any().y > 0
    }

    pub fn down(&self) -> bool {
        self.any(&[KeyCode::ArrowDown, KeyCode::KeyS], &[]) || self.nav_any().y < 0
    }

    pub fn left(&self) -> bool {
        self.any(&[KeyCode::ArrowLeft, KeyCode::KeyA], &[]) || self.nav_any().x < 0
    }

    pub fn right(&self) -> bool {
        self.any(&[KeyCode::ArrowRight, KeyCode::KeyD], &[]) || self.nav_any().x > 0
    }

    /// Cross alone (a pad's Start is not it): the in-game screens that
    /// pause on Start.
    pub fn cross(&self) -> bool {
        self.any(
            &[KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ],
            &[GamepadButton::South],
        )
    }

    pub fn confirm(&self) -> bool {
        self.any(
            &[KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ],
            &[GamepadButton::South, GamepadButton::Start],
        )
    }

    pub fn back(&self) -> bool {
        self.any(
            &[KeyCode::Escape, KeyCode::Backspace],
            &[GamepadButton::East, GamepadButton::North],
        )
    }

    /// The keyboard's Circle (O), which the screens that take Circle read
    /// beside a pad's ([`Self::pad_circle`]).
    pub fn circle_key(&self) -> bool {
        self.any(&[KeyCode::KeyO], &[])
    }

    /// The controller that pressed Select this frame (P on the keyboard,
    /// as in a game; a pad's Select): the original's question "Exit to
    /// Main Menu?" ([`crate::exit_ask`]).
    pub fn selected_by(&self) -> Option<Device> {
        if self.keyboard() && self.keys.just_pressed(KeyCode::KeyP) {
            return Some(Device::Keyboard);
        }
        self.pads()
            .find(|(_, g)| g.just_pressed(GamepadButton::Select))
            .map(|(e, _)| Device::Pad(e))
    }

    /// Square held (L on the keyboard, a pad's West), not while L1 is (the
    /// User Setup's code checker owns the face buttons then).
    pub fn square_held(&self) -> bool {
        let l1 = (self.keyboard() && self.keys.pressed(KeyCode::KeyQ))
            || self
                .pads()
                .any(|(_, g)| g.pressed(GamepadButton::LeftTrigger));
        !l1 && ((self.keyboard() && self.keys.pressed(KeyCode::KeyL))
            || self.pads().any(|(_, g)| g.pressed(GamepadButton::West)))
    }

    /// Any key or pad button this frame, or a pad's direction: what a
    /// screen's handler takes as a message (the idle demo's timer on Main
    /// restarts on each, `FUN_0029e700`).
    pub fn any_press(&self) -> bool {
        (self.keyboard() && self.keys.get_just_pressed().next().is_some())
            || self
                .pads()
                .any(|(_, g)| g.get_just_pressed().next().is_some())
            || self.nav_any() != IVec2::ZERO
    }

    /// Start this frame (Enter, or a pad's Start): the title's only key
    /// (`FUN_002d3cf0`, message 0x3d).
    pub fn start(&self) -> bool {
        self.any(&[KeyCode::Enter], &[GamepadButton::Start])
    }

    /// A pad's Circle (East), which the menus otherwise take for Back.
    pub fn pad_circle(&self) -> bool {
        self.any(&[], &[GamepadButton::East])
    }

    pub fn pause(&self) -> bool {
        self.any(&[KeyCode::Escape, KeyCode::KeyP], &[GamepadButton::Start])
    }

    /// The controller that pressed pause (Start) this frame.
    pub fn paused_by(&self) -> Option<Device> {
        if self.keyboard() && self.keys.any_just_pressed([KeyCode::Escape, KeyCode::KeyP]) {
            return Some(Device::Keyboard);
        }
        self.pads()
            .find(|(_, g)| g.just_pressed(GamepadButton::Start))
            .map(|(e, _)| Device::Pad(e))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Page {
    Title,
    Main,
    PlayIt,
    /// The difficulty on the way into a game (class 105: Play It's Pick Up
    /// Game goes there first, `FUN_002bf428`).
    Difficulty,
    /// Who plays: each controller moves itself to Away, CPU or Home (User
    /// Setup's side chooser, each column its own controller's).
    Sides,
    ChooseTeam(usize),
    /// Choose Players (class 0x1d): both sides on one screen, each picked
    /// by its own people ([`crate::choose`]).
    ChoosePlayers,
    ChooseCourt,
    /// Choose Court's "Play Game / My Rules" dialog (`FUN_0024f508`, text
    /// 0xd05), over the court screen.
    PlayGameAsk,
    MyRules,
    /// My Rules' "Cancel Changes and Return to Choose Court?" (`FUN_002cb548`,
    /// text 0xe14), over the rules screen; Yes returns to Choose Court
    /// with the rules as they were.
    RulesCancelAsk,
    /// Be A Legend's "Choose Baller": the ten careers; then, for an empty
    /// one, which baller starts it.
    Careers,
    /// Choose Baller's dialog over a slot (`FUN_002cdf10`): Male, Female,
    /// Premade, Cancel on an empty one; Continue Career, Edit Baller, Edit
    /// Moves, Cancel on a career.
    CareerAsk(usize),
    NewCareer(usize),
    /// The premade picker's "CREATE / %1S?" (`FUN_00274260`: YES, EDIT,
    /// CANCEL) for career `slot`.
    PremadeAsk(usize),
    /// Be A Legend's map: the crew, Edit Baller and the six cities; then
    /// one city's events.
    Story,
    City(usize),
    /// An event's intro, before its game (or its bracket).
    EventIntro(usize),
    StoryCrew,
    /// Be A Legend: your team, the three starters and the bench.
    StoryTeam,
    /// After a story win: take a player from the beaten team
    /// (`updateteam.txt`, the game's class 96).
    UpdateTeam(UpdateStage),
    /// Rewards (class 20, [`crate::rewards_view`]): the categories' cards
    /// and the trophies on one screen (`Menus::rewards`), and its question
    /// or notice (`Menus::reward_ask`).
    Rewards,
    RewardAsk,
    /// A mode finished: the game's end boards (classes 59 and 60): 0 the
    /// League Challenge, 1 Be A Legend.
    Completed(usize),
    /// The League Challenge's board then Choose Team's "join a team" (class
    /// 0x4d, [`crate::join_view`]) for the challenge team's created
    /// ballers (`Menus::join`).
    JoinTeam,
    /// Street School: the 26 lessons.
    School,
    /// The League Challenge: the regions, then one region's matches.
    League,
    /// The League Challenge's Choose Team (class 0x19): the person's team
    /// for the challenge, kept in the save (`FUN_001ee3d8`, user `+0x4a`).
    LeagueTeam,
    DevelopmentRecipient(usize),
    LeagueRegion(usize),
    /// My Team: Create A Baller and Create Team.
    MyTeam,
    /// The six Create A Baller slots, then one baller's editor.
    Ballers,
    /// Create A Baller's dialog over a slot (Choose Baller's in mode 1,
    /// `FUN_002cdf10`): Male, Female, Premade, Cancel on an empty one; Edit
    /// Baller, Edit Moves, Cancel on a baller.
    BallerAsk(usize),
    EditBaller(usize),
    /// Create Baller's wizard (`FUN_00273a08(screen, 3, gender)`, classes
    /// 0x4a and 0x4b): the baller's slot and the panel
    /// ([`crate::cab_view::Step`]'s index).
    Wizard(usize, usize),
    /// The wizard's "CREATE / %1S?" (`FUN_00274260`: YES, EDIT, CANCEL)
    /// over the name panel, for the baller's slot.
    WizardAsk(usize),
    /// A question over the wizard or Edit Baller (`Menus::cab_ask`), for
    /// the baller's slot.
    CabAsk(usize),
    /// Edit Moves (`crate::edit_moves`): step 1 on a category's slots (the
    /// category; the cursor is the slot), step 2 on the new move for a slot
    /// (the category and the slot; the cursor is the move), and step 2's
    /// question (`edit_moves::Panel::ask`).
    MoveSlots(usize),
    MovePick(usize, usize),
    MoveAsk(usize, usize),
    /// Options' "Do you want to cancel your changes?" (`options.txt` 46,
    /// `FUN_002a4d18`'s Triangle when an option changed).
    OptionsAsk,
    /// The rewrite's own settings (a window or full screen, the Wide
    /// picture, vibration), which Adjust Screen opens: the original's
    /// Options screen has no rows for them.
    PcSettings,
    /// A Be A Legend tournament's bracket (by event).
    Bracket(usize),
    /// Options → Game Data (class 97, [`crate::game_data`]), its delete
    /// lists over it (0 names, 1 created teams, 2 created ballers) and its
    /// questions and notices (`Menus::data_ask`).
    GameData,
    Delete(usize),
    DataAsk,
    /// Inside Vol. 2: its main page, What's New In 2, Bobbito Garcia, and
    /// The Moves (every move's film-room clip, by kind).
    Inside,
    WhatsNew,
    Bobbito,
    TheMoves(usize),
    /// High Scores (class 10, [`crate::score_view`]): one table a tab, the
    /// cursor the tab.
    HighScores,
    /// After Choose Sides: the name each controller plays under.
    Users,
    /// The created teams, one team's editor, and the player pick for a row.
    Teams,
    /// A new team's players (class 0x35, `crate::create_team`).
    CreateTeam,
    EditTeam(usize),
    PickPlayer,
    Options,
    /// Options → Trax (class 0x30, [`crate::trax_view`]): the radio's
    /// rows; and its "Do you want to cancel your changes?".
    Trax,
    TraxAsk,
    /// Options → Controller Layout: each player's layout.
    Layouts,
    /// Host or join a match over the network ([`crate::online`]).
    Online,
    MatchBrowser,
    Controls,
    /// The credits' screen (class 9, [`crate::credits`]).
    Credits,
    /// A team's credits rolling (class 8 NuFX's, 7 the developer's).
    CreditsRoll(crate::credits::Team),
}

/// Update Team's steps, as the game's screen runs them (`FUN_002d7238`,
/// `FUN_002d6c00`, `FUN_002d6fd0`): the question, the beaten team's list,
/// your roster's positions, and the two confirmations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UpdateStage {
    /// "Would you like to update your team with a player from the defeated
    /// team?"
    Ask,
    /// "Select a player to add to your team."
    Pick,
    /// "Select a roster position for your new player."
    Slot,
    /// "Accept roster changes and continue?"
    Accept,
    /// "Exit without making changes?"
    Exit,
}

/// An Update Team under way: the event it follows, the beaten players and
/// the choices so far.
#[derive(Clone, Debug, Default)]
pub struct UpdateTeam {
    pub ev: usize,
    pub beaten: Vec<u16>,
    pub pick: Option<u16>,
    pub slot: usize,
}

/// Where the menus are: a stack of pages with a cursor each.
#[derive(Resource)]
pub struct Menus {
    stack: Vec<(Page, usize)>,
    /// Players picked on Choose Players (database records).
    picks: Vec<usize>,
    /// The crew picked on Story Crew (player records).
    crew: Vec<u16>,
    /// Where a played story match returns to (set by the match's outcome
    /// screen; the next entry into the menus goes straight there).
    return_page: Option<Page>,
    /// The controllers connected, as of this frame (for Choose Sides).
    devices: Vec<Device>,
    /// Online, what this machine picks for the lobby's match on the menus
    /// (`lobby.rs`): its sides' teams and players, and for the creator the
    /// court and rules; `online_picked` once done.
    pub(crate) online_pick: Option<crate::lobby::Duties>,
    pub(crate) online_picked: bool,
    /// Online, a side another machine picks for: its first person's seat
    /// and whether its picks are in (`online::mirror_picks` keeps them and
    /// the side's team and three in the setup).
    pub(crate) online_others: [Option<(usize, bool)>; 2],
    /// The controllers whose Choose Sides column is choosing (state 10):
    /// the one that confirmed into it, and those that pressed START since
    /// (the rest read "Press START To Enter Game", state 1).
    joined: Vec<Device>,
    /// Whose buttons the prompts name (`pads::PadNav::style`, as of this
    /// frame).
    pub(crate) style: crate::pads::ButtonStyle,
    /// The Online page's choices.
    pub(crate) online: crate::online::OnlineForm,
    /// Go online: set by the menus, taken by `online::start_from_menu`.
    pub(crate) go_online: Option<crate::online::OnlineArgs>,
    /// A name being typed on the name page, and for which seat.
    pub(crate) new_name: String,
    naming: Option<usize>,
    /// User Setup continues to league's player chooser for its selected person.
    league_user_setup: bool,
    /// User Setup continues to the selected person's Street School lessons.
    school_user_setup: bool,
    /// "Join a team"'s turn, grid and dialogs.
    pub(crate) join: crate::join_view::Join,
    /// A created baller joined a league team: the rosters take it on.
    teams_changed: bool,
    /// Square held on a screen with a Help legend: the Help overlay is up
    /// (`crate::help_view`).
    pub(crate) help: bool,
    /// Game Data's question or notice.
    data_ask: Option<crate::game_data::Ask>,
    /// Where the Rewards screen stands, and its question.
    rewards: crate::rewards_view::State,
    reward_ask: Option<crate::rewards_view::Ask>,
    /// Trax's settings as the screen opened on them, and the first row
    /// its list shows.
    trax_before: Option<street_sim::records::Settings>,
    trax_top: usize,
    /// A deleted baller's record, to put back as the disc has it.
    restore: Option<usize>,
    /// Update Team, offered after a story win.
    update: Option<UpdateTeam>,
    /// A DVD page's movies, asked for by its keys (`DAT_003fdf44`): the
    /// page goes forward into itself through them.
    pub(crate) dvd: Option<&'static [&'static str]>,
    /// The credits rolling ([`crate::credits::run`]).
    pub(crate) credits: Option<crate::credits::Roll>,
    controller_player: usize,
    controller_view: u8,
    /// User Setup: each seat's column state (`users::Column`), as the
    /// original's column widget runs them.
    pub(crate) user_stages: Vec<UserStage>,
    /// Choose Team: the first grid row each side's list shows.
    pub(crate) team_scroll: [usize; 2],
    /// Choose Players: both sides' pickers.
    pub(crate) players: crate::choose::Screen,
    /// The crew's Create Team (class 0x62): its grid and picks, and the
    /// ten local ballers, drawn once a run as the original's static list
    /// (`FUN_001ed1f8`, `0x3ec9c8`).
    pub(crate) crew_pick: crate::create_team::Picker,
    /// The crew's boards over its grid (Welcome, the keyboards, Choose
    /// Logo) and what they hold: the city and name typed, the logo's tile.
    pub(crate) crew_stage: crate::create_team::Stage,
    pub(crate) crew_city: String,
    pub(crate) crew_name: String,
    pub(crate) crew_logo: usize,
    /// The lit row of Create Team's questions (`create_team::Stage::Cancel`,
    /// `Stage::Save`).
    pub(crate) team_ask: usize,
    /// My Team's Create Team: its grid and picks.
    pub(crate) team_pick: crate::create_team::Picker,
    /// The card widget's question over My Team's Create Team
    /// (`create_team::Stage::Save`), and whether the card's save of the
    /// team is under way (its end decides between My Team and the grid).
    pub(crate) team_card: crate::create_team::CardAsk,
    pub(crate) team_saving: bool,
    /// The help line left empty after the card's question closed unsaved:
    /// only a cancelled board sets it again (`FUN_00280f98`).
    pub(crate) team_help_gone: bool,
    pub(crate) locals: Vec<usize>,
    /// My Rules: the records the rows edit, and the rules as they were when
    /// the screen opened (Cancel puts them back).
    pub(crate) rules_edit: crate::myrules::Edit,
    rules_before: Option<Rules>,
    /// Choose Court: the court the Random tile shows (`+0x108`), redrawn
    /// every quarter second while the tile is highlighted.
    pub(crate) court_random: Option<usize>,
    /// Front-end updates so far (the Random tile's clock).
    frames: u64,
    /// Create Baller's wizard: list 0x14's first row shown, the icons'
    /// first shown, and the career the baller starts (Be A Legend's
    /// dialog; `None` from My Team).
    pub(crate) wizard_top: usize,
    pub(crate) wizard_icon_top: usize,
    pub(crate) wizard_career: Option<usize>,
    /// Edit Baller's panel (`cab_view::Edit`'s index).
    pub(crate) edit_panel: usize,
    /// Edit Moves' lists, question, clip and way in.
    pub(crate) moves: crate::edit_moves::Panel,
    /// The question `Page::CabAsk` shows.
    pub(crate) cab_ask: Option<crate::cab_view::CabAsk>,
    /// Options: the list's first row shown, and the settings as they were
    /// when the screen opened (Cancel puts them back).
    pub(crate) options_top: usize,
    pub(crate) options_before: Option<street_sim::records::Settings>,
    /// The premade picker opened from Create A Baller's dialog, for this
    /// slot (`None` from Be A Legend's).
    pub(crate) premade_team: Option<usize>,
    /// The pages as the keys found them this update: a save to the card
    /// keeps them standing under its dialogs.
    shown: Vec<(Page, usize)>,
    /// The card's work asked for this update, with the pages it stands
    /// over ([`card_hold`]).
    card_request: Option<(crate::card::Op, Vec<(Page, usize)>)>,
    /// The pages to show once the card is done.
    after_card: Option<Vec<(Page, usize)>>,
    /// Game Data deleted something: leaving it saves (`FUN_00299110`'s
    /// `+0xf0`).
    data_deleted: bool,
    /// Rewards bought a card: leaving it saves (`FUN_002c1370`'s `+0x2b0`).
    rewards_bought: bool,
    /// Select's question over the screen, its lit row
    /// ([`crate::exit_ask`]).
    pub(crate) exit_ask: Option<usize>,
    /// A menu sound a screen plays itself (the question's 3 as it opens),
    /// for `audio::menu_sounds`.
    pub(crate) cue: Option<usize>,
    /// What the screen's leaving adds to the next change's hold, ms
    /// (`transition::Change::hold_ms`), and whether its keys play the
    /// change's sound themselves where the pair alone does not say
    /// (`transition::Change::whoosh`; Select's Yes).
    pub(crate) hold_ms: f32,
    pub(crate) whoosh: Option<bool>,
}

/// A User Setup column's state after Choose Sides (`FUN_00246880`'s 2, 3,
/// 4, 7, 9 and 11/12).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UserStage {
    /// "Enter Name", Yes or No.
    Ask {
        yes: bool,
    },
    /// The saved names: "Enter New" first, then the names.
    Pick {
        at: usize,
        first: usize,
    },
    /// The on-screen keyboard.
    Typing,
    Named,
    Guest,
    Ready,
}

impl UserStage {
    const START: Self = UserStage::Ask { yes: true };
}

/// Difficulty's rows top to bottom (`FUN_00284280` registers elements 17,
/// 16, 15: Legendary, Mad Game, Got Game; `FUN_00284848` sets 2, 1, 0).
const DIFFICULTY_ROWS: [Difficulty; 3] = [Difficulty::Hard, Difficulty::Medium, Difficulty::Easy];

/// Where Difficulty goes (`FUN_00284848` by the mode Play It set): Choose
/// Baller (class 0x36) in Be A Legend, User Setup for the League Challenge
/// (0xe), else User Setup's side chooser (0xd).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AfterDifficulty {
    ChooseBaller,
    LeagueUserSetup,
    ChooseSides,
}

/// `from` is the page under Difficulty with its cursor.
fn after_difficulty(from: Option<(Page, usize)>) -> AfterDifficulty {
    match from {
        Some((Page::PlayIt, row)) if PLAY_IT_ROWS.get(row) == Some(&3) => {
            AfterDifficulty::ChooseBaller
        }
        Some((Page::PlayIt, row)) if PLAY_IT_ROWS.get(row) == Some(&2) => {
            AfterDifficulty::LeagueUserSetup
        }
        _ => AfterDifficulty::ChooseSides,
    }
}

/// What Choose Baller's dialog row does (`FUN_002cd9c8`'s answers 0x56 and
/// 0x52): on an empty slot Male, Female and Premade create the baller
/// (classes 0x4a, 0x4b, 0x4c), on a career Continue Career (the map, 0x38),
/// Edit Baller (0x4f) and Edit Moves (0x55); the last row cancels.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CareerChoice {
    NewBaller,
    Continue,
    EditBaller,
    EditMoves,
    Cancel,
}

fn career_choice(used: bool, row: usize) -> CareerChoice {
    match (used, row) {
        (false, 0..=2) => CareerChoice::NewBaller,
        (true, 0) => CareerChoice::Continue,
        (true, 1) => CareerChoice::EditBaller,
        (true, 2) => CareerChoice::EditMoves,
        _ => CareerChoice::Cancel,
    }
}

/// The page after a side's team is picked on Choose Team (one screen for
/// both sides, class 0x1b): the other side's, then Choose Players.
fn after_team(side: usize) -> (Page, usize) {
    if side == 0 {
        (Page::ChooseTeam(1), 0)
    } else {
        (Page::ChoosePlayers, 0)
    }
}

/// The row Difficulty opens on (`FUN_00284280` selects the current one).
fn difficulty_row(d: Difficulty) -> usize {
    DIFFICULTY_ROWS.iter().position(|&x| x == d).unwrap_or(2)
}

/// The rows' words without the disc (`difficulty.txt` 3, 2, 1).
fn difficulty_label(d: Difficulty) -> &'static str {
    match d {
        Difficulty::Hard => "Legendary",
        Difficulty::Medium => "Mad Game",
        Difficulty::Easy => "Got Game",
    }
}

impl Default for Menus {
    fn default() -> Self {
        Self {
            stack: vec![(Page::Title, 0)],
            picks: Vec::new(),
            crew: Vec::new(),
            return_page: None,
            devices: vec![Device::Keyboard],
            joined: Vec::new(),
            online_pick: None,
            online_picked: false,
            online_others: [None; 2],
            style: Default::default(),
            online: Default::default(),
            go_online: None,
            new_name: String::new(),
            naming: None,
            league_user_setup: false,
            school_user_setup: false,
            help: false,
            data_ask: None,
            trax_before: None,
            rewards: Default::default(),
            reward_ask: None,
            trax_top: 0,
            restore: None,
            update: None,
            dvd: None,
            credits: None,
            controller_player: 0,
            controller_view: 0,
            user_stages: Vec::new(),
            team_scroll: [0, 0],
            players: Default::default(),
            crew_pick: Default::default(),
            crew_stage: Default::default(),
            team_ask: 0,
            crew_city: String::new(),
            crew_name: String::new(),
            crew_logo: 0,
            team_pick: Default::default(),
            team_card: crate::create_team::CardAsk::Create,
            team_saving: false,
            team_help_gone: false,
            locals: Vec::new(),
            rules_edit: Default::default(),
            rules_before: None,
            court_random: None,
            frames: 0,
            wizard_top: 0,
            wizard_icon_top: 0,
            wizard_career: None,
            edit_panel: 0,
            moves: Default::default(),
            cab_ask: None,
            options_top: 0,
            options_before: None,
            premade_team: None,
            shown: Vec::new(),
            card_request: None,
            after_card: None,
            data_deleted: false,
            rewards_bought: false,
            exit_ask: None,
            cue: None,
            hold_ms: 0.0,
            whoosh: None,
            join: Default::default(),
            teams_changed: false,
        }
    }
}

impl Menus {
    pub(crate) fn top(&self) -> (Page, usize) {
        *self.stack.last().expect("never empty")
    }

    /// The page a dialog page sits over, with its cursor.
    pub(crate) fn under_dialog(&self) -> Option<(Page, usize)> {
        match self.top().0 {
            Page::PlayGameAsk
            | Page::RulesCancelAsk
            | Page::CareerAsk(_)
            | Page::BallerAsk(_)
            | Page::PremadeAsk(_)
            | Page::WizardAsk(_)
            | Page::CabAsk(_)
            | Page::OptionsAsk
            | Page::Delete(_)
            | Page::DataAsk
            | Page::TraxAsk
            | Page::RewardAsk
            | Page::DevelopmentRecipient(_)
            | Page::MoveAsk(..) => self.stack.get(self.stack.len().checked_sub(2)?).copied(),
            _ => None,
        }
    }

    fn push(&mut self, page: Page, cursor: usize) {
        self.stack.push((page, cursor));
    }

    fn cursor(&mut self) -> &mut usize {
        &mut self.stack.last_mut().expect("never empty").1
    }

    /// The game's own save to the card (`FUN_002d8460(.., 0x3f)`): the
    /// pages the keys found stay under the card's dialogs, and the pages
    /// this update went on to show come once the card is done.
    fn save_to_card(&mut self) {
        self.card_request = Some((crate::card::Op::Autosave, self.shown.clone()));
    }

    /// Game Data's own work on the card (its Save and Load rows), over the
    /// screen.
    fn card_work(&mut self, op: crate::card::Op) {
        self.card_request = Some((op, self.stack.clone()));
    }

    /// Whether the screen the load back opens saves to the card itself
    /// (Update Team, a completion board, a finished tournament's bracket):
    /// then the load back does not (`FUN_0029d5a8`).
    pub fn next_saves_itself(&self, story: &street_sim::story::Story) -> bool {
        match self.return_page {
            Some(Page::UpdateTeam(_) | Page::Completed(_)) => true,
            Some(Page::Bracket(ev)) => tournament_over(story, ev),
            _ => false,
        }
    }

    /// A credits roll leaves for Options (`FUN_002d7d40(.., 0x28)`), past
    /// the credits' screen.
    pub(crate) fn leave_credits(&mut self) {
        while matches!(self.top().0, Page::CreditsRoll(_) | Page::Credits) && self.stack.len() > 1 {
            self.stack.pop();
        }
        self.credits = None;
    }

    /// The title alone (the idle demo's turn, and where its game comes
    /// back to: `FUN_0029d920`, mode 0).
    pub(crate) fn back_to_title(&mut self) {
        self.stack = vec![(Page::Title, 0)];
        self.return_page = None;
        self.dvd = None;
    }

    /// After a story match: fall back at `page` instead of the main menu.
    pub fn set_return(&mut self, page: Option<Page>) {
        self.return_page = page;
    }

    /// After a story win: Update Team with these beaten players, then back
    /// to the event's page.
    pub fn offer_update(&mut self, ev: usize, beaten: Vec<u16>) {
        self.update = Some(UpdateTeam {
            ev,
            beaten,
            ..Default::default()
        });
        self.return_page = Some(Page::UpdateTeam(UpdateStage::Ask));
    }

    /// Create Baller's name panel after its keyboard: DONE on the first
    /// name goes to the last's, on the last to the icons; Cancel goes back
    /// a panel (the first name's to Skills).
    pub(crate) fn wizard_name_step(&mut self, field: crate::typing::Field, done: bool) {
        use crate::cab_view::Step;
        let Some(top) = self.stack.last_mut() else {
            return;
        };
        let Page::Wizard(slot, _) = top.0 else {
            return;
        };
        let to = match (field, done) {
            (Field::CabFirst, true) => Step::Last,
            (Field::CabLast, true) => Step::Icon,
            (Field::CabLast, false) => Step::First,
            _ => Step::Skills,
        };
        *top = (Page::Wizard(slot, to.index()), 0);
    }

    /// Moves Update Team to another step, its cursor at `cursor`.
    fn update_step(&mut self, stage: UpdateStage, cursor: usize) {
        if let Some(top) = self.stack.last_mut() {
            *top = (Page::UpdateTeam(stage), cursor);
        }
    }
}

#[derive(Component)]
struct TitleText;

#[derive(Component)]
struct BodyText;

#[derive(Component)]
struct HintText;

/// The menu's background image, swapped per page in the draw pass.
#[derive(Component)]
struct MenuBg;

pub struct MenuPlugin;

impl Plugin for MenuPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Menus>()
            .add_systems(Update, crate::rewards::repair_card_unlocks)
            .add_systems(Update, crate::ballers::refresh_outfit_extras)
            .add_systems(Update, crate::options::follow_switched_songs)
            .init_resource::<crate::card::Card>()
            .init_resource::<CheatEntries>()
            .init_resource::<crate::cheats::Settings>()
            .init_resource::<Message>()
            .init_resource::<ballers::Editing>()
            .init_resource::<ballers::Preview>()
            .init_resource::<teams::TeamEditing>()
            .init_resource::<Typing>()
            .init_resource::<look::Labels>()
            .init_resource::<crate::rewards_view::BigCard>()
            .init_resource::<crate::rewards_view::Turn>()
            .add_systems(OnEnter(Screen::Menu), spawn_menu)
            .add_systems(PostStartup, apply_settings)
            .add_systems(
                Update,
                (
                    menu_bg,
                    typing::type_text,
                    commit_name,
                    typing::show_keyboard,
                    reset_cheats_at_home,
                    card_menu,
                    crate::attract::idle,
                    navigate
                        .run_if(crate::transition::idle)
                        .run_if(crate::attract::takes_presses),
                    card_hold,
                    reload_data,
                    ballers::apply_commit,
                    restore_records,
                    teams::apply_commit,
                    moves_panel,
                    reward_card_picture,
                    watch,
                    draw.run_if(crate::transition::draws),
                    ballers::preview,
                )
                    .chain()
                    .before(crate::transition::TransitionSystems)
                    .run_if(in_state(Screen::Menu)),
            )
            .add_systems(
                OnExit(Screen::Menu),
                |mut t: ResMut<crate::transition::Transition>| t.last = None,
            );
    }
}

/// Main-menu activation clears match cheat selectors in the original.
fn reset_cheats_at_home(
    menus: Res<Menus>,
    mut cheats: ResMut<crate::cheats::Settings>,
    mut was_home: Local<bool>,
) {
    let home = menus.top().0 == Page::Main;
    if home && !*was_home {
        cheats.reset();
    }
    *was_home = home;
}

/// The background image, spawned once the art has decoded and kept on the
/// page (the draw pass swaps its handle).
fn menu_bg(mut commands: Commands, art: Option<Res<art::Art>>, has: Query<(), With<MenuBg>>) {
    let Some(art) = art else {
        return;
    };
    if has.iter().next().is_none() {
        let mut node =
            ImageNode::new(art.backdrop(art::backdrop::MAIN)).with_mode(NodeImageMode::Stretch);
        // Sit the art behind the text: dim it so the lists stay readable.
        node.color = Color::srgba(0.55, 0.55, 0.62, 0.5);
        commands.spawn((
            node,
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                ..default()
            },
            MenuBg,
            DespawnOnExit(Screen::Menu),
        ));
    }
}

fn spawn_menu(mut commands: Commands, mut menus: ResMut<Menus>) {
    // Back from a match: the story pages resume where they were. (Picking
    // for an online lobby opens the menus on its own pages.)
    if menus.top().0 != Page::Title && menus.online_pick.is_none() {
        menus.stack.truncate(2);
        if let Some(page) = menus.return_page.take() {
            // A city's page sits on the map, a bracket on its city's page,
            // and Update Team on the event's page.
            let ev = match page {
                Page::Bracket(ev) => Some(ev),
                Page::UpdateTeam(_) => menus.update.as_ref().map(|u| u.ev),
                _ => None,
            };
            if matches!(
                page,
                Page::Story
                    | Page::Bracket(_)
                    | Page::UpdateTeam(_)
                    | Page::City(_)
                    | Page::Completed(1)
            ) {
                menus.push(Page::PlayIt, PLAY_IT_LEGEND);
                menus.push(Page::Difficulty, difficulty_row(Difficulty::Easy));
                menus.push(Page::Careers, 0);
            }
            if matches!(
                page,
                Page::Bracket(_) | Page::UpdateTeam(_) | Page::City(_) | Page::Completed(1)
            ) {
                menus.push(Page::Story, 0);
            }
            if page == Page::Completed(0) {
                menus.push(Page::League, 0);
            }
            if let Some(ev) = ev {
                let city = street_sim::story::city_of(ev);
                let row = street_sim::story::city_events(city)
                    .iter()
                    .position(|&e| e == ev)
                    .unwrap_or(0);
                menus.push(Page::City(city), row);
                if matches!(page, Page::UpdateTeam(_))
                    && street_sim::story::EVENTS[ev].kind
                        == street_sim::story::EventKind::Tournament
                {
                    menus.push(Page::Bracket(ev), 0);
                }
            }
            // Back from Street School: Play It on its row.
            let row = if page == Page::PlayIt {
                PLAY_IT_SCHOOL
            } else {
                0
            };
            menus.push(page, row);
        }
    }
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::srgb(0.04, 0.04, 0.07)),
            ..default()
        },
        IsDefaultUiCamera,
        DespawnOnExit(Screen::Menu),
    ));
    // The game's own art lands behind the menus once it decodes (see
    // `menu_bg`, updated every frame on this screen).
    let text = |size: f32| TextFont {
        font_size: bevy::text::FontSize::Px(size),
        ..default()
    };
    commands
        .spawn((
            Node {
                width: Val::Percent(100.0),
                height: Val::Percent(100.0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                padding: UiRect::all(Val::Px(40.0)),
                row_gap: Val::Px(24.0),
                ..default()
            },
            DespawnOnExit(Screen::Menu),
        ))
        .with_children(|root| {
            root.spawn((
                Text::new(""),
                text(44.0),
                TextColor(Color::srgb(1.0, 0.82, 0.25)),
                TitleText,
            ));
            root.spawn((
                Text::new(""),
                text(24.0),
                TextLayout::justify(Justify::Left),
                BodyText,
            ));
            root.spawn((
                Text::new(""),
                text(18.0),
                TextColor(Color::srgb(0.6, 0.6, 0.65)),
                HintText,
            ));
        });
}

/// What each page lists.
#[allow(clippy::too_many_arguments)]
fn items(
    page: Page,
    menus: &Menus,
    disc: &Disc,
    setup: &MatchSetup,
    save: &story::Save,
    (sound, pads, opts, _songs): (
        &AudioSettings,
        &crate::pads::PadSettings,
        &street_sim::records::Settings,
        &[street_audio::music::Song],
    ),
    editing: &ballers::Editing,
    team: &teams::TeamEditing,
) -> Vec<String> {
    match page {
        Page::Title => vec!["Press Start".into()],
        Page::Main => [
            "Play It",
            "My Team",
            "Rewards",
            "Options",
            "High Scores",
            "Inside Vol. 2",
        ]
        .map(String::from)
        .to_vec(),
        Page::PlayIt => [
            "Pick Up Game",
            "League Challenge",
            "Be A Legend",
            "Street School",
        ]
        .map(String::from)
        .to_vec(),
        // User Setup draws its columns (`users.rs`); Choose Sides has no
        // rows, the name page one per seat.
        Page::Sides => vec![String::new()],
        Page::Difficulty => DIFFICULTY_ROWS
            .iter()
            .map(|&d| difficulty_label(d).to_owned())
            .collect(),
        Page::Careers => crate::careers::slots(&save.story, disc)
            .iter()
            .map(|slot| match slot {
                crate::careers::Slot::Empty => String::new(),
                crate::careers::Slot::Career { name, .. } => name.clone(),
            })
            .collect(),
        Page::CareerAsk(slot) => {
            let used = save.story.career_at(slot).is_some();
            if used {
                ["Continue Career", "Edit Baller", "Edit Moves", "Cancel"]
            } else {
                ["Male", "Female", "Premade", "Cancel"]
            }
            .map(String::from)
            .to_vec()
        }
        // Create Baller's premade picker draws its own panel (`cab_view.rs`).
        Page::NewCareer(_) => Vec::new(),
        // The dialogs' rows (drawn by `pause::dialog`): YES, EDIT, CANCEL.
        Page::PremadeAsk(_) | Page::WizardAsk(_) => {
            ["YES", "EDIT", "CANCEL"].map(String::from).to_vec()
        }
        Page::Story => crate::map_view::ROW_CITY
            .iter()
            .map(|&c| street_sim::story::CITIES[c].to_owned())
            .collect(),
        Page::City(c) => story::city_items(&save.story, disc, c),
        // The intros have no rows: Continue and Back are their legends.
        Page::EventIntro(_) => Vec::new(),
        // The Create Teams keep their own grids (`crate::create_team`).
        Page::StoryCrew | Page::CreateTeam => Vec::new(),
        Page::StoryTeam => {
            let mut list = story::team_items(&save.story, disc);
            list.push("Back".into());
            list
        }
        Page::UpdateTeam(stage) => {
            let u = menus.update.clone().unwrap_or_default();
            story::update_items(stage, &u.beaten, u.pick, &save.story, disc)
        }
        Page::League => story::league_region_items(&save.story, disc),
        // Class 0x19's grid, a row a cell as Choose Team's.
        Page::LeagueTeam => crate::chooseteam::league_grid(disc, &save.story)
            .iter()
            .map(|c| {
                c.team
                    .and_then(|t| disc.teams.get(t))
                    .map(|t| t.full_name())
                    .unwrap_or_default()
            })
            .collect(),
        // The recipients' rows are the dialog's (`rewards_view::recipient_pieces`),
        // one for each created baller: the cursor runs over them.
        Page::DevelopmentRecipient(_) => crate::rewards::development_recipients(&save.story)
            .iter()
            .map(|&r| disc.players[usize::from(r)].name())
            .collect(),
        Page::LeagueRegion(r) => story::league_match_items(&save.story, r, disc),
        Page::Rewards | Page::RewardAsk => Vec::new(),
        Page::Completed(mode) => {
            story::completed_items(&save.story, disc, mode, setup.rules.difficulty)
        }
        // "Join a team"'s grid keeps its own cursor (`Menus::join`).
        Page::JoinTeam => Vec::new(),
        // Street School's game starts at once (its lessons are an in-game
        // menu, `crate::school_menu`).
        Page::School => Vec::new(),
        // Choose Team's grid (`chooseteam.rs`): a row a cell, blanks
        // included, so the cursor is the cell.
        Page::ChooseTeam(_) => crate::chooseteam::grid(disc, &save.story)
            .iter()
            .map(|c| {
                c.team
                    .and_then(|t| disc.teams.get(t))
                    .map(|t| t.full_name())
                    .unwrap_or_default()
            })
            .collect(),
        // Choose Players' pickers keep their own cursors (`crate::choose`).
        Page::ChoosePlayers => Vec::new(),
        // Choose Court's twelve widgets in registration order: the courts,
        // then Random Select (`FUN_00267f10`).
        Page::ChooseCourt => (0..crate::choosecourt::CELLS)
            .map(|k| {
                if k == crate::choosecourt::RANDOM {
                    "Random Select".to_owned()
                } else {
                    disc.courts
                        .get(k)
                        .map(|c| format!("{:<20} {}", c.name, c.location))
                        .unwrap_or_default()
                }
            })
            .collect(),
        Page::PlayGameAsk => ["Play Game", "My Rules"].map(String::from).to_vec(),
        // Set Challenge's registered rows (`setchallenge.txt` 1, 2, 3, 4,
        // 36); the values are the screen's own widgets (`myrules.rs`).
        Page::MyRules => [
            "Game Mode",
            "Play To",
            "Spot Points",
            "Shot Clock",
            "Play Game",
        ]
        .map(String::from)
        .to_vec(),
        Page::RulesCancelAsk => ["Yes", "No"].map(String::from).to_vec(),
        Page::MyTeam => vec!["Create A Baller".into(), "Create Team".into()],
        Page::Ballers => ballers::slot_items(disc, &save.story),
        Page::BallerAsk(slot) => if save
            .story
            .ballers
            .iter()
            .any(|b| usize::from(b.slot) == slot)
        {
            vec!["Edit Baller", "Edit Moves", "Cancel"]
        } else {
            vec!["Male", "Female", "Premade", "Cancel"]
        }
        .into_iter()
        .map(String::from)
        .collect(),
        Page::Teams => teams::slot_items(&save.story),
        Page::EditTeam(slot) => team
            .team
            .as_ref()
            .map(|t| teams::rows(t, disc, slot < save.story.teams.len()))
            .unwrap_or_default(),
        Page::PickPlayer => teams::pick_items(disc, &save.story),
        // Edit Moves draws its own lists (`crate::edit_moves`).
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) => Vec::new(),
        Page::CabAsk(_) => Vec::new(),
        Page::HighScores => Vec::new(),
        Page::Inside => [
            "What's New In 2",
            "The Moves",
            "Bobbito Garcia",
            "Vol. 2 Trailer",
            "SSX 3 Preview",
        ]
        .map(String::from)
        .to_vec(),
        Page::WhatsNew => [
            "Play All",
            "New Game",
            "Take On The League",
            "Be A Legend",
            "Vol. 2 Rewards",
        ]
        .map(String::from)
        .to_vec(),
        Page::Bobbito => ["New York City", "Vol. 2"].map(String::from).to_vec(),
        // The Moves draws its own list (`crate::edit_moves`, browsing).
        Page::TheMoves(_) => Vec::new(),
        Page::Bracket(ev) => story::bracket_items(&save.story, disc, ev),
        Page::Users => vec![String::new(); setup.seats.len().max(1)],
        Page::EditBaller(_) => editing
            .baller
            .as_ref()
            .map(|b| ballers::rows(b, editing.skill, editing))
            .unwrap_or_default(),
        // Create Baller's panels draw their own lists (`cab_view.rs`).
        Page::Wizard(..) => Vec::new(),
        Page::Options => options::items(opts, sound, setup.rules.difficulty),
        Page::OptionsAsk => Vec::new(),
        Page::PcSettings => options::pc_items(opts, pads),
        Page::Trax => crate::trax_view::rows(opts)
            .iter()
            .map(|r| format!("{r:?}"))
            .collect(),
        Page::TraxAsk => Vec::new(),
        Page::Layouts => vec![
            format!("Player {}", menus.controller_player + 1),
            options::LAYOUT_NAMES[usize::from(opts.layouts[menus.controller_player])].into(),
            ["Offense", "Defense", "Advanced"][usize::from(menus.controller_view)].into(),
            format!("Vibration  {}", if pads.vibration { "ON" } else { "OFF" }),
            "Back".into(),
        ],
        Page::GameData => [
            "Save",
            "Load",
            "Delete Name",
            "Delete Created Team",
            "Delete Created Baller",
            "Exit",
        ]
        .map(String::from)
        .to_vec(),
        // The card's delete lists (`FUN_00258a48`): a user's name, a
        // team's name, a baller's slot and name.
        Page::Delete(kind) => match kind {
            0 => save
                .story
                .records
                .users
                .iter()
                .map(|u| u.name.clone())
                .collect(),
            1 => save.story.teams.iter().map(|t| t.name.clone()).collect(),
            _ => save
                .story
                .ballers
                .iter()
                .enumerate()
                .map(|(k, b)| crate::game_data::baller_row(k, &b.first_name, &b.last_name))
                .collect(),
        },
        Page::DataAsk => Vec::new(),
        Page::Online => menus.online.rows(),
        Page::MatchBrowser => {
            let mut rows: Vec<String> = menus
                .online
                .matches
                .iter()
                .map(|m| {
                    format!(
                        "{}  {}  {}/6  {}",
                        m.name,
                        m.court,
                        m.players,
                        if m.finished {
                            "Finished"
                        } else if m.started {
                            "Playing"
                        } else {
                            "Lobby"
                        }
                    )
                })
                .collect();
            if rows.is_empty() {
                rows.push(
                    if menus.online.answered {
                        "No matches"
                    } else {
                        "Looking for matches..."
                    }
                    .into(),
                );
            }
            rows.push("Back".into());
            rows
        }
        Page::Controls => {
            // The controls of whatever was touched last.
            let mut rows = menus.style.controls();
            rows.push("Back".into());
            rows
        }
        // Their layouts' words and the roll's own (`crate::credits`).
        Page::Credits | Page::CreditsRoll(_) => Vec::new(),
    }
}

/// The mode a set of rules was made from (by its distinguishing setting).
pub(crate) fn mode_of(r: &Rules) -> Mode {
    Mode::ALL
        .into_iter()
        .rev()
        .find(|&m| {
            let base = Rules::mode(m);
            base.scoring == r.scoring
                && base.dunks_only == r.dunks_only
                && base.tricks == r.tricks
                && base.gamebreakers_score == r.gamebreakers_score
                && std::mem::discriminant(&base.goal) == std::mem::discriminant(&r.goal)
        })
        .unwrap_or(Mode::Normal)
}

fn title(page: Page, disc: &Disc, _setup: &MatchSetup) -> String {
    match page {
        Page::Title => String::new(),
        Page::Main => "Main Menu".into(),
        Page::PlayIt => "Play It".into(),
        Page::Difficulty => "Difficulty".into(),
        Page::Sides => "Choose Sides".into(),
        Page::ChooseTeam(0) => "Choose Team".into(),
        Page::ChooseTeam(_) => "Choose Opponent".into(),
        Page::ChoosePlayers => "Choose Players".into(),
        Page::ChooseCourt | Page::PlayGameAsk => "Choose Court".into(),
        Page::MyRules | Page::RulesCancelAsk => "My Rules".into(),
        Page::Careers | Page::CareerAsk(_) => "Choose Baller".into(),
        Page::NewCareer(_) | Page::PremadeAsk(_) => "Choose Baller".into(),
        Page::Story => "The Map".into(),
        Page::City(c) => street_sim::story::CITIES[c].into(),
        Page::EventIntro(ev) => story::intro_title(disc, ev),
        Page::StoryCrew | Page::CreateTeam => "Create Team".into(),
        Page::StoryTeam => "Your Team".into(),
        Page::UpdateTeam(_) => "Update Team".into(),
        Page::League => "League Challenge".into(),
        Page::LeagueTeam => "Choose Team".into(),
        Page::DevelopmentRecipient(_) => "Which Baller Receives Development Points?".into(),
        Page::School => "Street School".into(),
        Page::Rewards | Page::RewardAsk => "Rewards".into(),
        Page::Completed(_) => "Congratulations!".into(),
        Page::JoinTeam => "Join Team".into(),
        Page::LeagueRegion(r) => story::REGION_NAMES[r].into(),
        Page::MyTeam => "My Team".into(),
        Page::Ballers | Page::BallerAsk(_) | Page::EditBaller(_) => "Create A Baller".into(),
        Page::Wizard(..) | Page::WizardAsk(_) | Page::CabAsk(_) => "Create Baller".into(),
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) => "Edit Moves".into(),
        Page::GameData | Page::Delete(_) | Page::DataAsk => "Game Data".into(),
        Page::Inside => "Inside Vol. 2".into(),
        Page::WhatsNew => "What's New In 2".into(),
        Page::Bobbito => "Bobbito On...".into(),
        Page::TheMoves(_) => "The Moves".into(),
        Page::Bracket(_) => "Tournament".into(),
        Page::HighScores => disc
            .score
            .get(12)
            .cloned()
            .unwrap_or_else(|| "High Scores".into()),
        Page::Users => "Enter Names".into(),
        Page::Teams | Page::EditTeam(_) => "Create Team".into(),
        Page::PickPlayer => "Choose A Player".into(),
        Page::Options | Page::OptionsAsk => "Options".into(),
        Page::PcSettings => "PC Settings".into(),
        Page::Trax | Page::TraxAsk => "Trax".into(),
        Page::Layouts => "Controller Layout".into(),
        Page::Online => crate::look::ONLINE_TITLE.into(),
        Page::MatchBrowser => "Matches".into(),
        Page::Controls => "Controls".into(),
        Page::Credits => "Credits".into(),
        Page::CreditsRoll(_) => String::new(),
    }
}

/// The game's own strings for a page, where it has them (file and index in
/// `XLOC_FE`): the title.
fn game_title(page: Page) -> Option<(&'static str, usize)> {
    Some(match page {
        Page::Main => ("mainmenu.txt", 0),
        Page::PlayIt => ("playit.txt", 0),
        Page::ChooseTeam(_) | Page::LeagueTeam => ("common_fe.txt", 0),
        Page::Difficulty => ("difficulty.txt", 0),
        Page::Sides => ("entername.txt", 0),
        // The layout's title, `common.txt` 21.
        Page::ChoosePlayers => ("common.txt", 21),
        Page::ChooseCourt | Page::PlayGameAsk => ("choosecrt.txt", 0),
        Page::MyRules | Page::RulesCancelAsk => ("setchallenge.txt", 0),
        Page::MyTeam => ("myteam.txt", 0),
        Page::Ballers | Page::BallerAsk(_) => ("myteam.txt", 1),
        // Layout 79's title, "Edit Baller" (0x300c).
        Page::EditBaller(_) => ("editplyr.txt", 12),
        // `FUN_00293a78` names the editor's screen "The Moves" (0x360e).
        Page::TheMoves(_) => ("editplyr_moves.txt", 14),
        // `FUN_00290bb8` names the editor's screen "Edit Moves" (0x3600).
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) => ("editplyr_moves.txt", 0),
        // Layout 61's title (0x2003).
        Page::Wizard(..) | Page::WizardAsk(_) => ("createplyr.txt", 3),
        Page::Teams | Page::EditTeam(_) | Page::StoryCrew | Page::CreateTeam => {
            ("createteam.txt", 0)
        }
        Page::Options | Page::OptionsAsk => ("options.txt", 0),
        Page::Trax => ("bigradio.txt", 0),
        Page::Layouts => ("controlconfig.txt", 0),
        Page::UpdateTeam(_) => ("updateteam.txt", 0),
        Page::Completed(0) => ("nbatcend.txt", 0),
        // `FUN_00271730`: element 0x3a's 0x113.
        Page::JoinTeam => ("common_fe.txt", 19),
        // Layout 60's title is "Be A Legend" (0x1f0a).
        Page::Completed(_) => ("bealegendend.txt", 10),
        Page::Inside => ("dvdcontent.txt", 0),
        Page::WhatsNew => ("dvdnewin2.txt", 0),
        Page::Bobbito => ("dvdbobbito.txt", 0),
        Page::Story => ("mapscreen.txt", 0),
        Page::Careers | Page::CareerAsk(_) => ("storymainmenu.txt", 0),
        Page::League | Page::LeagueRegion(_) => ("choosergn.txt", 0),
        Page::City(_) => ("mapscreen.txt", 1),
        Page::Users => ("entername.txt", 0),
        _ => return None,
    })
}

/// ... an entry's label (where ours is the game's entry under another
/// order) ...
fn game_label(page: Page, k: usize) -> Option<(&'static str, usize)> {
    match (page, k) {
        (Page::Main, 0..=5) => Some(("mainmenu.txt", k + 1)),
        // The game's own rows (the layouts' prop 17, `dvd*.txt`).
        (Page::Inside, 0) => Some(("dvdcontent.txt", 1)),
        (Page::Inside, 1) => Some(("dvdcontent.txt", 3)),
        (Page::Inside, 2) => Some(("dvdcontent.txt", 5)),
        (Page::Inside, 3) => Some(("dvdcontent.txt", 2)),
        (Page::Inside, 4) => Some(("dvdcontent.txt", 4)),
        (Page::WhatsNew, 0) => Some(("dvdnewin2.txt", 7)),
        (Page::WhatsNew, 1) => Some(("dvdnewin2.txt", 2)),
        (Page::WhatsNew, 2) => Some(("dvdnewin2.txt", 4)),
        (Page::WhatsNew, 3) => Some(("dvdnewin2.txt", 5)),
        (Page::WhatsNew, 4) => Some(("dvdnewin2.txt", 8)),
        (Page::Bobbito, 0) => Some(("dvdbobbito.txt", 1)),
        (Page::Bobbito, 1) => Some(("dvdbobbito.txt", 2)),
        (Page::MyTeam, 0 | 1) => Some(("myteam.txt", k + 1)),
        (Page::Difficulty, 0..=2) => Some(("difficulty.txt", [3, 2, 1][k])),
        (Page::PlayIt, 0) => Some(("playit.txt", 2)),
        (Page::PlayIt, 1) => Some(("playit.txt", 3)),
        (Page::PlayIt, 2) => Some(("playit.txt", 4)),
        (Page::PlayIt, 3) => Some(("playit.txt", 1)),
        _ => None,
    }
}

/// ... and the help line for the highlighted entry.
fn game_help(page: Page, k: usize) -> Option<(&'static str, usize)> {
    Some(match (page, k) {
        (Page::Main, 0..=3) => ("main_help.txt", k),
        // Create Team's 0x1d09 for the crew (`FUN_0027e458`).
        (Page::StoryCrew, _) => ("createteam.txt", 9),
        (Page::CreateTeam, _) => ("createteam.txt", 8),
        (Page::Main, 4) => ("main_help.txt", 5),
        (Page::Main, 5) => ("mainmenu.txt", 7),
        (Page::Inside, 0) => ("dvdcontent.txt", 6),
        (Page::Inside, 1) => ("dvdcontent.txt", 8),
        (Page::Inside, 2) => ("dvdcontent.txt", 10),
        (Page::Inside, 3) => ("dvdcontent.txt", 7),
        (Page::Inside, 4) => ("dvdcontent.txt", 9),
        (Page::WhatsNew, 0) => ("dvdnewin2.txt", 16),
        (Page::WhatsNew, 1..=4) => ("dvdnewin2.txt", 9),
        (Page::Bobbito, 0) => ("dvdbobbito.txt", 5),
        (Page::Bobbito, 1) => ("dvdbobbito.txt", 6),
        (Page::Users, _) => ("main_help.txt", 14),
        (Page::JoinTeam, _) => ("jointeam.txt", 0),
        // Layout 9's help line, 0x4403.
        (Page::Credits, _) => ("creditsintro.txt", 3),
        (Page::HighScores, k) => ("score.txt", crate::score_view::TABS.get(k)?.help),
        (Page::Story, 2..) => ("mapscreen.txt", 10),
        (Page::UpdateTeam(stage), _) => (
            "updateteam.txt",
            match stage {
                UpdateStage::Ask => 4,
                UpdateStage::Pick => 5,
                UpdateStage::Slot => 6,
                UpdateStage::Accept => 2,
                UpdateStage::Exit => 1,
            },
        ),
        (Page::PlayIt, 0) => ("main_help.txt", 10),
        (Page::PlayIt, 1) => ("main_help.txt", 11),
        (Page::PlayIt, 2) => ("main_help.txt", 12),
        (Page::PlayIt, 3) => ("main_help.txt", 9),
        (Page::Options, k) if options::help(k).is_some() => {
            ("options.txt", options::help(k).unwrap_or(44))
        }
        (Page::GameData, k) => ("gamedata.txt", [6, 7, 8, 9, 10, 11][k.min(5)]),
        (Page::Difficulty, 0..=2) => ("difficulty.txt", [6, 5, 4][k]),
        (Page::ChooseTeam(_) | Page::LeagueTeam, _) => ("common_fe.txt", 9),
        (Page::ChoosePlayers, _) => ("chooseplyr.txt", 2),
        // `FUN_00268ce0`: 0xd09 on the Random tile, 0xd04 elsewhere.
        (Page::ChooseCourt | Page::PlayGameAsk, crate::choosecourt::RANDOM) => ("choosecrt.txt", 9),
        (Page::ChooseCourt | Page::PlayGameAsk, _) => ("choosecrt.txt", 4),
        (Page::Ballers, _) | (Page::MyTeam, 0) => ("main_help.txt", 8),
        (Page::MyTeam, _) | (Page::Teams, _) => ("main_help.txt", 7),
        (Page::EditTeam(_), 0) => ("createteam.txt", 16),
        (Page::EditTeam(_), 1) => ("createteam.txt", 17),
        (Page::EditTeam(_), 2) => ("createteam.txt", 11),
        (Page::EditTeam(_), teams::PLAYER_ROW..teams::SAVE_ROW) | (Page::PickPlayer, _) => {
            ("createteam.txt", 8)
        }
        _ => return None,
    })
}

/// Help for the entries the game has no line for.
fn own_help(page: Page, k: usize) -> &'static str {
    match (page, k) {
        (Page::Main, 6) => "Play people over the network.",
        (Page::EditBaller(_), 0 | 1) | (Page::EditTeam(_), 0 | 1) => {
            "Left/Right for a name, or Enter to type one."
        }
        (Page::EditTeam(_), teams::SAVE_ROW) => "Save the team for Pick Up games.",
        (Page::PcSettings, k) => options::pc_help(k),
        (Page::StoryTeam, _) => {
            "The first three start. Cross/Enter moves a player between the starters and the bench."
        }
        (Page::Controls, _) => "Plug in an Xbox or PlayStation controller at any time.",
        (Page::EditBaller(_), ballers::MOVES_ROW) => "Choose the baller's dunks, tricks and shots.",
        (Page::EditTeam(_), teams::DELETE_ROW) => "Delete this team.",
        (Page::Sides, _) => {
            "Each controller: Left to Away, Right to Home. Nobody: the computer plays."
        }
        _ => "",
    }
}

fn navigate(
    mut input: MenuInput,
    mut menus: ResMut<Menus>,
    mut setup: ResMut<MatchSetup>,
    disc: Res<Disc>,
    mut save: ResMut<story::Save>,
    mut running: ResMut<story::Running>,
    mut message: ResMut<Message>,
    mut next: ResMut<NextState<Screen>>,
    mut exit: MessageWriter<AppExit>,
    (mut sound, mut pads, mut opts, sounds): (
        ResMut<AudioSettings>,
        ResMut<crate::pads::PadSettings>,
        ResMut<options::Options>,
        Res<crate::audio::Sounds>,
    ),
    mut editing: ResMut<ballers::Editing>,
    (names, mut cheat_entries, mut cheats): (
        Res<ballers::Names>,
        ResMut<CheatEntries>,
        ResMut<crate::cheats::Settings>,
    ),
    mut team_editing: ResMut<teams::TeamEditing>,
    (mut typing, mut commands, mut session): (
        ResMut<Typing>,
        Commands,
        Option<ResMut<crate::session::Session>>,
    ),
    look: Option<Res<Look>>,
    card: Res<crate::card::Card>,
) {
    // Typing a name owns the keys until it ends, and the memory card's
    // dialogs while it works (and the update its last answer ends it).
    if typing.busy() || card.flow.is_some() || card.closed {
        return;
    }
    menus.shown = menus.stack.clone();
    menus.devices = input.devices();
    menus.style = input.style();
    // Pads that went away leave their seats.
    let devices = menus.devices.clone();
    setup.seats.retain(|(d, _)| devices.contains(d));
    let (page, cursor) = menus.top();
    *input.only = listens_to(page, &setup.seats);
    // Select's question takes the keys until it is answered; Select opens
    // it where the screen asks (`FUN_0024bfa0`).
    if let Some(lit) = menus.exit_ask {
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            menus.exit_ask = Some(crate::exit_ask::step(lit, dy));
        } else if input.cross() {
            menus.exit_ask = None;
            if lit == crate::exit_ask::YES
                && let Some(asked) = select_asks(page, &menus, &save.story, &setup.seats, None)
            {
                message.0.clear();
                exit_to_main(
                    asked,
                    &mut menus,
                    &mut save,
                    (&mut opts, &mut sound, &mut setup.rules.difficulty),
                    &mut editing,
                    (&mut running, &mut cheats),
                );
            }
        }
        menus.help = false;
        return;
    }
    // Square held on a screen the Help table lists: the overlay shows and
    // holds the screen's keys until it is let go (`FUN_00250000`).
    let help = input.square_held()
        && screen_class(page, &menus, &save.story, &disc)
            .is_some_and(|class| !crate::help_view::rows(class).is_empty());
    menus.help = help;
    if help {
        return;
    }
    if let Some(device) = input.selected_by()
        && select_asks(page, &menus, &save.story, &setup.seats, Some(device)).is_some()
    {
        menus.exit_ask = Some(crate::exit_ask::OPENS_ON);
        menus.cue = Some(crate::exit_ask::OPEN_SOUND);
        return;
    }
    // The rewrite's own: Select on the Main Menu opens Online, as its
    // legend says (Vj, 2026-10-21). Main asks no Select question.
    if page == Page::Main && input.selected_by().is_some() {
        menus.push(Page::Online, 0);
        return;
    }
    if page == Page::JoinTeam {
        join_keys(&input, &mut menus, &mut save);
        return;
    }
    // Street School's User Setup done: its one game loads (`FUN_001dba68`
    // mode 1), the lesson menu inside it.
    if page == Page::School {
        menus.stack.pop();
        start_school(&disc, &mut setup, &mut running, &mut message);
        if running.0.is_some() {
            team_set(session.as_deref_mut(), &setup, 1);
            next.set(Screen::Loading);
        }
        return;
    }
    // The original User Setup checker owns face buttons while L1 is held.
    // Keep each controller's rolling code independent of the other people.
    if page == Page::Users {
        let mut consumed = false;
        for &device in &devices {
            let entry = cheat_entries.0.entry(device).or_default();
            let held = match device {
                Device::Keyboard => input.keys.pressed(KeyCode::KeyQ),
                Device::Pad(e) => input
                    .gamepads
                    .get(e)
                    .is_ok_and(|(_, p)| p.pressed(GamepadButton::LeftTrigger)),
            };
            let mode = if setup.watch { 1 } else { 2 };
            if held != entry.is_held() {
                entry.handle_action(mode, if held { 0x31 } else { 0x35 });
            }
            for (button, key, pad) in [
                (
                    crate::cheats::Button::Triangle,
                    KeyCode::KeyI,
                    GamepadButton::North,
                ),
                (
                    crate::cheats::Button::Circle,
                    KeyCode::KeyJ,
                    GamepadButton::East,
                ),
                (
                    crate::cheats::Button::Square,
                    KeyCode::KeyL,
                    GamepadButton::West,
                ),
                (
                    crate::cheats::Button::Cross,
                    KeyCode::KeyK,
                    GamepadButton::South,
                ),
            ] {
                let pressed = match device {
                    Device::Keyboard => input.keys.just_pressed(key),
                    Device::Pad(e) => input
                        .gamepads
                        .get(e)
                        .is_ok_and(|(_, p)| p.just_pressed(pad)),
                };
                if !pressed {
                    continue;
                }
                let result = entry.handle_action(mode, button.action());
                consumed |= result.consumed;
                match result.feedback {
                    Some(crate::cheats::Feedback::Accepted(cheat)) => {
                        cheats.apply(cheat);
                        crate::rewards::grant_cheat(&mut save.story, &disc, cheat);
                        save.store();
                        message.0 = format!("{} activated", cheat.name());
                    }
                    Some(crate::cheats::Feedback::Rejected) => message.0 = "Invalid code".into(),
                    None => {}
                }
            }
            consumed |= held && mode != 1;
        }
        if consumed {
            return;
        }
    } else {
        cheat_entries.0.clear();
    }
    if page == Page::Sides {
        // Each controller moves its own column's side: up (or left) to
        // AWAY, down (or right) to HOME, back to none (`FUN_00245b88`
        // state 10: the chooser's nodes 0x38, 0x39, 0x3a).
        // A controller that has not joined joins on START (or Cross),
        // its column turning to the chooser.
        let mut joining = false;
        for &d in &devices {
            if !menus.joined.contains(&d) {
                if input.confirm_of(d) {
                    menus.joined.push(d);
                    joining = true;
                }
                continue;
            }
            let step = input.step_of(d) + input.vertical_of(d);
            if step != 0 {
                move_seat(&mut setup.seats, d, step.signum());
            }
        }
        if !joining && input.confirm() && !setup.seats.is_empty() {
            begin_names(&mut menus, &mut setup);
        } else if input.back() {
            menus.stack.pop();
        }
        return;
    }
    if page == Page::StoryCrew {
        crew_navigate(&input, &mut menus, &mut save, &mut typing, look.as_deref());
        return;
    }
    if page == Page::CreateTeam {
        team_navigate(
            &input,
            &mut menus,
            &mut team_editing,
            &mut save,
            &disc,
            &mut typing,
            look.as_deref(),
        );
        return;
    }
    // Choose Region: the region list runs across, round (`FUN_0026c430`);
    // a region's teams step over their tiles to the boss's card.
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    if page == Page::League && dx != 0 {
        let n = street_sim::story::REGION_MATCHES.len() as i32;
        *menus.cursor() = (cursor as i32 + dx).rem_euclid(n) as usize;
        return;
    }
    if let Page::LeagueRegion(r) = page
        && (dx != 0 || dy != 0)
    {
        let teams = street_sim::story::REGION_MATCHES[r].len();
        *menus.cursor() = crate::region_view::step(cursor, dx, dy, teams);
        return;
    }
    if matches!(page, Page::League | Page::LeagueRegion(_)) && (input.up() || input.down()) {
        return;
    }
    if page == Page::ChoosePlayers {
        match players_navigate(&input, &mut menus, &mut setup, &devices) {
            PlayersOutcome::Finished if menus.players.mode != crate::choose::Mode::PickUp => {
                // The League Challenge's and Be A Legend's game (modes 4 and 5:
                // `FUN_0026a860` goes on to the loading screen, class 0x2c).
                if running.0.is_some() {
                    next.set(Screen::Loading);
                }
            }
            PlayersOutcome::Finished if menus.online_pick.as_ref().is_some_and(|d| !d.court) => {
                menus.online_picked = true;
            }
            PlayersOutcome::Finished => {
                // The list opens on node 0x5a, the top-left tile
                // (`FUN_0021f6a8`).
                menus.court_random = None;
                menus.push(Page::ChooseCourt, crate::choosecourt::FIRST);
            }
            PlayersOutcome::Left => {
                if menus.players.mode != crate::choose::Mode::PickUp {
                    running.0 = None;
                }
                menus.stack.pop();
            }
            PlayersOutcome::Picking => {}
        }
        return;
    }
    if page == Page::Users {
        users_navigate(
            &input,
            &mut menus,
            &mut setup,
            &mut save,
            &disc,
            &mut typing,
            &mut message,
            &mut cheats,
        );
        return;
    }
    let count = items(
        page,
        &menus,
        &disc,
        &setup,
        &save,
        (&sound, &pads, &opts.0, sounds.songs()),
        &editing,
        &team_editing,
    )
    .len();
    // The map and a city's page: Circle is Edit Baller (`FUN_0029f4b0`;
    // O on the keyboard, or a pad's Circle).
    if matches!(page, Page::Story | Page::City(_)) && (input.circle_key() || input.pad_circle()) {
        edit_star(&mut editing, &save.story, &disc, &mut menus, &mut message);
        return;
    }
    // The premade picker: L1 and R1 step its list (`FUN_00277fa8`: 0x31,
    // 0x33), each step loading the premade (`FUN_00278098`).
    if let Page::NewCareer(_) = page {
        let turn = input.shoulder();
        if turn != 0 {
            let n = street_sim::story::BALLER_SLOTS as i32;
            let to = (cursor as i32 + turn).rem_euclid(n) as usize;
            *menus.cursor() = to;
            show_premade(&mut editing, &disc, to);
            return;
        }
        if input.up() || input.down() || input.left() || input.right() {
            return;
        }
    }
    // High Scores (`FUN_002d30f0`): L1 and R1, or left and right, step the
    // tabs; Cross and Triangle go back to the Main Menu.
    if page == Page::HighScores {
        let dir = input.shoulder() + i32::from(input.right()) - i32::from(input.left());
        if dir != 0 {
            *menus.cursor() = crate::score_view::step(cursor, dir.signum());
        } else if input.confirm() || input.back() {
            menus.stack.pop();
        }
        return;
    }
    // Game Data's question: up and down between YES and NO; Triangle
    // closes it (Cross answers below).
    if page == Page::DataAsk {
        let rows = menus
            .data_ask
            .as_ref()
            .map_or(1, |a| 1 + usize::from(a.asks()));
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            *menus.cursor() = (cursor as i32 + dy).clamp(0, rows as i32 - 1) as usize;
            return;
        }
        if input.back() {
            menus.data_ask = None;
            menus.stack.pop();
            return;
        }
    }
    if matches!(page, Page::Rewards | Page::RewardAsk) {
        rewards_keys(
            &input,
            &mut menus,
            &mut save,
            &disc,
            look.as_deref(),
            &mut message,
        );
        return;
    }
    if matches!(page, Page::Trax | Page::TraxAsk) {
        trax_keys(&input, &mut menus, &mut opts, &mut commands);
        return;
    }
    if matches!(page, Page::Options | Page::OptionsAsk) {
        options_keys(
            &input,
            &mut menus,
            &mut save,
            (&mut opts, &mut sound, &pads, &mut setup),
        );
        return;
    }
    if matches!(page, Page::Credits | Page::CreditsRoll(_)) {
        credits_keys(&input, &mut menus);
        return;
    }
    if let Page::CabAsk(_) = page {
        cab_ask_keys(
            &input,
            &mut menus,
            &mut editing,
            &mut save,
            &disc,
            &mut message,
        );
        return;
    }
    if let Page::EditBaller(_) = page {
        editor_keys(
            &input,
            &mut menus,
            &mut editing,
            &mut save,
            &mut message,
            &names,
            look.as_deref(),
        );
        return;
    }
    if let Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) | Page::TheMoves(_) = page {
        moves_keys(
            &input,
            &mut menus,
            &mut editing,
            &mut save,
            &disc,
            &mut message,
        );
        return;
    }
    if let Page::Wizard(slot, at) = page {
        wizard_keys(
            &input,
            &mut menus,
            &mut editing,
            &save.story,
            &mut typing,
            &mut message,
            &names,
            look.as_deref(),
            (slot, crate::cab_view::Step::at(at)),
        );
        return;
    }
    if input.back() {
        if page == Page::Completed(0) {
            save.story.restart_league();
            save.store();
        }
        // The boards leave on any key the way Cross does, through the
        // card's save; so does a finished tournament's bracket.
        if matches!(page, Page::Completed(_)) {
            menus.stack.pop();
            menus.save_to_card();
            return;
        }
        if matches!(page, Page::Bracket(ev) if tournament_over(&save.story, ev)) {
            leave_bracket(&mut menus);
            return;
        }
        // Leaving the rewrite's settings keeps them for the next run.
        if page == Page::PcSettings {
            keep_options(&mut save, &opts, &sound, &pads, &setup);
        }
        if matches!(page, Page::NewCareer(_)) {
            editing.baller = None;
            menus.premade_team = None;
        }
        if matches!(page, Page::EditTeam(_)) {
            team_editing.team = None;
        }
        // Update Team steps back through its own steps.
        if let Page::UpdateTeam(stage) = page {
            match stage {
                UpdateStage::Ask => {
                    menus.update = None;
                    menus.stack.pop();
                }
                UpdateStage::Pick => menus.update_step(UpdateStage::Exit, 1),
                UpdateStage::Slot | UpdateStage::Exit => menus.update_step(UpdateStage::Pick, 0),
                UpdateStage::Accept => {
                    let slot = menus.update.as_ref().map_or(1, |u| u.slot);
                    menus.update_step(UpdateStage::Slot, slot)
                }
            }
            message.0.clear();
            return;
        }
        // Game Data after a delete: the card saves on the way out
        // (`FUN_00299110`'s Triangle, 0x25, with `+0xf0`).
        if page == Page::GameData && std::mem::take(&mut menus.data_deleted) {
            menus.stack.pop();
            menus.save_to_card();
            return;
        }
        // My Rules asks before it leaves (`FUN_002cb548`, the dialog
        // opening on No).
        if page == Page::MyRules {
            menus.push(Page::RulesCancelAsk, 1);
            return;
        }
        if menus.stack.len() >= 2 {
            menus.stack.pop();
        } else if page == Page::Title {
            exit.write(AppExit::Success);
        }
        return;
    }
    // Up/down follow the screen's authored navigation graph (kind-10
    // nodes' props 26/28) on Screen pages — the game's ends stay put
    // rather than wrap — and fall back to linear motion past the authored
    // rows or without the disc's layouts. The class mapping mirrors draw().
    let screen = match page {
        Page::Main | Page::Online => Some(3),
        Page::PlayIt => Some(5),
        Page::Difficulty => Some(105),
        Page::ChooseCourt => Some(33),
        Page::MyRules => Some(39),
        Page::MyTeam => Some(6),
        Page::Inside => Some(102),
        Page::WhatsNew => Some(103),
        Page::Bobbito => Some(104),
        Page::Layouts => Some(55),
        Page::Story | Page::City(_) => Some(56),
        Page::GameData => Some(crate::game_data::CLASS),
        _ => None,
    };
    let graph_step = |dir: i32| {
        screen
            .and_then(|class| look.as_deref()?.layouts.iter().find(|l| l.class == class))
            .and_then(|layout| {
                look::screen_step_filtered(layout, cursor, count, dir, |row| {
                    screen_row_available(row, count)
                })
            })
    };
    // Choose Team's grid: across and down its cells (`crate::chooseteam::step`),
    // scrolling the two rows shown.
    let grid_page = matches!(
        page,
        Page::ChooseTeam(_)
            | Page::LeagueTeam
            | Page::Careers
            | Page::City(_)
            | Page::UpdateTeam(UpdateStage::Pick | UpdateStage::Slot)
    );
    // Update Team's roster widgets (`FUN_002325e8`): five tiles two
    // across; the beaten team's as many as it has, the career's all five.
    if let Page::UpdateTeam(stage @ (UpdateStage::Pick | UpdateStage::Slot)) = page {
        let dx = i32::from(input.right()) - i32::from(input.left());
        let dy = i32::from(input.down()) - i32::from(input.up());
        let count = if stage == UpdateStage::Pick {
            menus.update.as_ref().map_or(0, |u| u.beaten.len())
        } else {
            crate::update_view::SLOTS
        };
        if dx != 0 || dy != 0 {
            *menus.cursor() = crate::update_view::step(cursor, dx, dy, count);
        }
    }
    // A city's events: list 0x1a's nodes (21..25) link straight down and
    // up a court at a time (the first's up and the last's down disabled),
    // keeping the slot the page is on where the court has one
    // (`FUN_002a2238`); left and right change the slot.
    if let Page::City(city) = page {
        let dy = i32::from(input.down()) - i32::from(input.up());
        let dx = i32::from(input.right()) - i32::from(input.left());
        if let Some(to) = crate::map_view::city_step(city, cursor, dx, dy) {
            *menus.cursor() = to;
        }
    }
    // Choose Baller: list 0x46's nodes link the ten slots down and up and
    // stop at the ends (the first slot's up and the last's down are mode
    // 0), three widgets a step (`+0xfa` 3).
    if matches!(page, Page::Careers | Page::Ballers) {
        let last = crate::careers::SLOTS - 1;
        if input.up() {
            *menus.cursor() = cursor.saturating_sub(1);
        }
        if input.down() {
            *menus.cursor() = (cursor + 1).min(last);
        }
    }
    if let Page::ChooseTeam(side) = page {
        let dx = i32::from(input.right()) - i32::from(input.left());
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dx != 0 || dy != 0 {
            let grid = crate::chooseteam::grid(&disc, &save.story);
            let to = crate::chooseteam::step(&grid, cursor, dx, dy);
            *menus.cursor() = to;
            menus.team_scroll[side] = crate::chooseteam::scroll(menus.team_scroll[side], to);
        }
    }
    if page == Page::LeagueTeam {
        let dx = i32::from(input.right()) - i32::from(input.left());
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dx != 0 || dy != 0 {
            let grid = crate::chooseteam::league_grid(&disc, &save.story);
            let to = crate::chooseteam::step(&grid, cursor, dx, dy);
            *menus.cursor() = to;
            menus.team_scroll[0] = crate::chooseteam::scroll(menus.team_scroll[0], to);
        }
    }
    if input.up() && count > 0 && !grid_page {
        *menus.cursor() = graph_step(-1).unwrap_or((cursor + count - 1) % count);
    }
    if input.down() && count > 0 && !grid_page {
        *menus.cursor() = graph_step(1).unwrap_or((cursor + 1) % count);
    }
    let step = match (input.left(), input.right()) {
        (true, false) => -1,
        (false, true) => 1,
        _ => 0,
    };
    // Choose Court's tiles link left and right too (list 0x61's nodes,
    // props 29..32); the Random tile redraws its court four times a
    // second while it is highlighted (`FUN_002688f0`).
    if page == Page::ChooseCourt {
        if step != 0 {
            let dir = if step < 0 { 2 } else { 3 };
            if let Some(to) = graph_step(dir) {
                *menus.cursor() = to;
            }
        }
        menus.frames += 1;
        let on_random = menus.top().1 == crate::choosecourt::RANDOM;
        if on_random
            && menus
                .frames
                .is_multiple_of(crate::choosecourt::RANDOM_TICKS)
        {
            let roll = (menus.frames as u32).wrapping_mul(2_654_435_761) ^ save.story.games;
            let last = menus.court_random;
            menus.court_random = crate::choosecourt::random_court(
                |c| crate::choosecourt::open(&save.story, &disc, c),
                last,
                roll,
            );
        }
    }
    // The map: left and right follow the city nodes' links (props 29..32),
    // skipping the locked cities (`FUN_0021f8f0`).
    if page == Page::Story && step != 0 {
        let dir = if step < 0 { 2 } else { 3 };
        let class = crate::map_view::CLASS;
        if let Some(to) = look
            .as_deref()
            .and_then(|l| l.layouts.iter().find(|l| l.class == class))
            .and_then(|layout| {
                look::screen_step_filtered(layout, cursor, 6, dir, |row| {
                    crate::map_view::row_available(&save.story, row)
                })
            })
        {
            *menus.cursor() = to;
        }
    }
    // My Rules: left and right edit the focused row's record
    // (`FUN_002c9740`), the arrows' flash counting down each update.
    if page == Page::MyRules {
        menus.rules_edit.tick();
        if step < 0 {
            menus.rules_edit.left(cursor);
        } else if step > 0 {
            menus.rules_edit.right(cursor);
        }
    }
    if step != 0 && !grid_page {
        if let Page::EditBaller(_) = page {
            let roll = (cursor as u32).wrapping_mul(2_654_435_761) ^ save.story.games;
            ballers::adjust(&mut editing, cursor, step, &names, roll);
        }
        if let Page::EditTeam(_) = page {
            teams::adjust(&mut team_editing, cursor, step, &disc, &save.story);
        }
        if page == Page::Online {
            menus.online.step(cursor, step);
        }
        if page == Page::PcSettings {
            options::pc_step(cursor, &mut opts.0, &mut pads);
        }
        if page == Page::Layouts {
            match cursor {
                0 => {
                    menus.controller_player =
                        (menus.controller_player as i32 + step).rem_euclid(4) as usize
                }
                1 => {
                    let l = &mut opts.0.layouts[menus.controller_player];
                    *l = (i32::from(*l) + step).rem_euclid(3) as u8;
                }
                2 => {
                    menus.controller_view =
                        (i32::from(menus.controller_view) + step).rem_euclid(3) as u8
                }
                3 => pads.vibration = !pads.vibration,
                _ => (),
            }
        }
    }
    // The title takes Start alone (`FUN_002d3cf0`: 0x3d goes on to Main,
    // Cross is eaten), its "Loading Demo..." too (measured: Start then
    // reached Main and the demo's turn was not taken).
    if page == Page::Title {
        if input.start() {
            message.0.clear();
            menus.push(Page::Main, 0);
        }
        return;
    }
    if !input.confirm() {
        return;
    }
    message.0.clear();

    // The story pages handle their own cursors first: crews, the Gym, and
    // starting events.
    match page {
        Page::Ballers => {
            // The slot's dialog (`FUN_002cdf10` in mode 1), its first row
            // lit; the rewrite has six records.
            if cursor < street_sim::story::BALLER_SLOTS {
                menus.push(Page::BallerAsk(cursor), 0);
            }
            return;
        }
        // Male and Female open the wizard (classes 0x4a, 0x4b), Premade
        // the picker (0x4c) for the slot; a baller's Edit Baller and Edit
        // Moves its editor (0x4f) and moves (0x55).
        Page::BallerAsk(slot) => {
            let used = save
                .story
                .ballers
                .iter()
                .any(|b| usize::from(b.slot) == slot);
            menus.stack.pop();
            match (used, cursor) {
                (false, 0 | 1) => {
                    start_wizard(slot, cursor == 1, &mut editing, &disc, &mut menus);
                    menus.wizard_career = None;
                }
                (false, 2) => {
                    show_premade(&mut editing, &disc, 0);
                    menus.premade_team = Some(slot);
                    menus.push(Page::NewCareer(slot), 0);
                }
                (true, 0 | 1) => {
                    open_baller(slot, &mut editing, &save.story, &disc, &mut menus);
                    if cursor == 1 && matches!(menus.top().0, Page::EditBaller(_)) {
                        open_moves(&mut menus, true);
                    }
                }
                _ => {}
            }
            return;
        }
        Page::MyTeam => {
            if cursor == 0 {
                menus.push(Page::Ballers, 0);
            } else {
                // `FUN_002a3c60`'s second row: Create Team (class 0x35) on a
                // new team.
                let (cities, names) = teams::presets(&disc);
                team_editing.team = Some(teams::new(&disc, &save.story, "New York", "Ballers"));
                team_editing.slot = save.story.teams.len();
                team_editing.cities = cities;
                team_editing.names = names;
                menus.team_pick = Default::default();
                menus.crew_stage = crate::create_team::Stage::Pick;
                menus.crew_logo = 0;
                menus.team_help_gone = false;
                menus.push(Page::CreateTeam, 0);
            }
            return;
        }
        Page::Teams => {
            let made = save.story.teams.len();
            let room = made < street_sim::story::TEAM_SLOTS;
            if cursor < made || (cursor == made && room) {
                let (cities, names) = teams::presets(&disc);
                team_editing.team = Some(match save.story.teams.get(cursor) {
                    Some(t) => t.clone(),
                    None => teams::new(&disc, &save.story, "New York", "Ballers"),
                });
                team_editing.slot = cursor;
                team_editing.cities = cities;
                team_editing.names = names;
                if cursor == made {
                    // A new team: its five from the grid first (class 0x35).
                    menus.team_pick = Default::default();
                    menus.push(Page::CreateTeam, 0);
                } else {
                    menus.push(Page::EditTeam(cursor), 0);
                }
            } else {
                menus.stack.pop();
            }
            return;
        }
        Page::EditTeam(slot) => {
            match cursor {
                0 => typing.field = Some(Field::TeamCity),
                1 => typing.field = Some(Field::TeamName),
                teams::PLAYER_ROW..teams::SAVE_ROW => {
                    team_editing.picking = cursor - teams::PLAYER_ROW;
                    let everyone = teams::everyone(&disc, &save.story);
                    let now = team_editing
                        .team
                        .as_ref()
                        .map(|t| usize::from(t.roster[team_editing.picking]));
                    let at = everyone.iter().position(|&r| Some(r) == now).unwrap_or(0);
                    menus.push(Page::PickPlayer, at);
                }
                teams::SAVE_ROW => match teams::commit(&mut team_editing, &mut save.story) {
                    Ok(()) => {
                        save.store();
                        team_editing.dirty = true;
                        message.0 = "Team saved".into();
                        menus.stack.pop();
                    }
                    Err(full) => message.0 = full.into(),
                },
                teams::DELETE_ROW if slot < save.story.teams.len() => {
                    save.story.teams.remove(slot);
                    save.store();
                    team_editing.team = None;
                    team_editing.dirty = true;
                    message.0 = "Team deleted".into();
                    menus.stack.pop();
                }
                _ => {}
            }
            return;
        }
        Page::PickPlayer => {
            let everyone = teams::everyone(&disc, &save.story);
            let row = team_editing.picking;
            if let (Some(t), Some(&record)) = (team_editing.team.as_mut(), everyone.get(cursor)) {
                teams::pick(t, row, record as u16);
            }
            menus.stack.pop();
            return;
        }
        // `FUN_002743d0`: YES makes the baller (the screen's +0x158; its
        // update saves it, `FUN_001e6888`, and leaves); EDIT goes back to
        // the first panel (`FUN_00274188(screen, 3)`); CANCEL stays.
        Page::WizardAsk(slot) => {
            menus.stack.pop();
            match cursor {
                0 => match ballers::commit(&mut editing, &mut save.story) {
                    Ok(()) => {
                        let career = menus.wizard_career.take();
                        menus.stack.pop();
                        match career {
                            // A new career's baller: the career, then its
                            // crew (class 0x62) before the map.
                            Some(c) => {
                                let record = street_sim::story::BALLER_RECORDS + slot as u16;
                                save.story.start_career(c, record);
                                save.store();
                                // Over Choose Baller (the premade picker,
                                // when EDIT opened the wizard, gives way).
                                if matches!(menus.top().0, Page::NewCareer(_)) {
                                    menus.stack.pop();
                                }
                                menus.push(Page::Story, 0);
                                begin_crew(&mut menus, &disc, &save.story, session.as_deref());
                                menus.push(Page::StoryCrew, 0);
                            }
                            None => {
                                save.store();
                                message.0 = "Baller created".into();
                            }
                        }
                        // The screen's update saves the new baller
                        // (`FUN_002745d8`: `FUN_001e6888`, then the card).
                        menus.save_to_card();
                    }
                    Err(e) => message.0 = e,
                },
                1 => {
                    if let Some(top) = menus.stack.last_mut() {
                        *top = (Page::Wizard(slot, 0), 0);
                    }
                    menus.wizard_top = 0;
                }
                _ => {}
            }
            return;
        }
        Page::EditBaller(_) => {
            if cursor < 2 {
                typing.field = Some(if cursor == 0 {
                    Field::BallerFirst
                } else {
                    Field::BallerLast
                });
            }
            if cursor == ballers::ROWS.len() - 1 {
                // On Skills, select the next skill to spend on.
                editing.skill = (editing.skill + 1) % ballers::SKILLS.len();
            } else if cursor == ballers::MOVES_ROW {
                open_moves(&mut menus, false);
            } else if cursor == ballers::SAVE_ROW {
                let before = save.story.ballers.clone();
                match ballers::commit(&mut editing, &mut save.story) {
                    Ok(()) => {
                        save.store();
                        message.0 = "Baller saved".into();
                        menus.stack.pop();
                        if save.story.ballers != before {
                            menus.save_to_card();
                        }
                    }
                    Err(e) => message.0 = e,
                }
            }
            return;
        }
        Page::Bracket(ev) => {
            // A decided tournament shows its final standings: Cross leaves
            // for Update Team when the title brought a player to take
            // (`FUN_002cd0f8`: the final's winner 0x25 goes to 0x60), else
            // through the card's save (`+0x11c` 0x38, `FUN_002d8460`).
            if tournament_over(&save.story, ev) {
                leave_bracket(&mut menus);
                return;
            }
            match cursor {
                0 => {
                    start_event(
                        ev,
                        input.confirmed_by().unwrap_or(Device::Keyboard),
                        &save,
                        &disc,
                        session.as_deref_mut(),
                        &mut setup,
                        &mut running,
                        &mut message,
                    );
                    if running.0.is_some() {
                        begin_story_players(
                            &mut menus,
                            &disc,
                            &setup,
                            &save.story,
                            crate::choose::Mode::Legend,
                        );
                        menus.push(Page::ChoosePlayers, 0);
                    }
                }
                1 => {
                    save.story.forfeit(ev);
                    save.store();
                    message.0 = "Tournament forfeited".into();
                    menus.stack.pop();
                }
                2 => {
                    menus.stack.pop();
                }
                _ => {}
            }
            return;
        }
        Page::Completed(mode) => {
            // The league board (`FUN_002a4898`): a challenge team holding
            // created ballers goes on to "join a team", which saves for
            // it; the challenge starts over either way (`FUN_001de8d0`).
            if mode == 0 {
                let joiners = crate::join_view::joiners(&disc, &save.story);
                save.story.restart_league();
                save.store();
                if !joiners.is_empty() {
                    menus.join = crate::join_view::Join::new(joiners);
                    *menus.stack.last_mut().unwrap() = (Page::JoinTeam, 0);
                    return;
                }
            }
            // Both boards save, then go back to Main (`FUN_002a4878`,
            // the Be A Legend board's: `FUN_002d7d40(screen, 3)`).
            menus.stack.truncate(2);
            menus.save_to_card();
            return;
        }
        Page::DevelopmentRecipient(id) => {
            if let Some(&recipient) =
                crate::rewards::development_recipients(&save.story).get(cursor)
            {
                if let Some(card) = disc.cards.iter().find(|c| c.id as usize == id)
                    && crate::rewards::buy_development(&mut save.story, card, recipient)
                {
                    save.store();
                    menus.rewards_bought = true;
                    message.0 = format!(
                        "+{} DP for {}",
                        card.index,
                        disc.players[usize::from(recipient)].name()
                    );
                    menus.stack.pop();
                } else {
                    message.0 = "Not enough Reward Points.".into();
                }
            } else {
                menus.stack.pop();
            }
            return;
        }
        Page::Careers => {
            // `FUN_002cdce8`'s confirm opens the slot's dialog
            // (`FUN_002cdf10`), its first row lit.
            if cursor < street_sim::story::CAREER_SLOTS {
                menus.push(Page::CareerAsk(cursor), 0);
            }
            return;
        }
        Page::CareerAsk(slot) => {
            let used = save.story.career_at(slot).is_some();
            menus.stack.pop();
            match career_choice(used, cursor) {
                // Male, Female and Premade (`FUN_002cd9c8`: classes 0x4a,
                // 0x4b, 0x4c, the Create Baller screens): the wizard for
                // a man or a woman in a star slot of the career's own,
                // the premade picker.
                CareerChoice::NewBaller if cursor < 2 => match career_star_slot(&save.story) {
                    Some(star) => {
                        start_wizard(star, cursor == 1, &mut editing, &disc, &mut menus);
                        menus.wizard_career = Some(slot);
                    }
                    None => message.0 = "No room for another baller.".into(),
                },
                CareerChoice::NewBaller => {
                    show_premade(&mut editing, &disc, 0);
                    menus.push(Page::NewCareer(slot), 0);
                }
                // Continue Career: the map (class 0x38); a career whose crew
                // is not picked yet goes to it first (`FUN_002cdf10`'s
                // class 0x62 while the career's bit 0 is clear).
                CareerChoice::Continue => {
                    save.story.switch_career(slot);
                    save.store();
                    menus.push(Page::Story, 0);
                    if save.story.crew.len() < CREW_SIZE {
                        begin_crew(&mut menus, &disc, &save.story, session.as_deref());
                        menus.push(Page::StoryCrew, 0);
                    }
                }
                // Edit Baller and Edit Moves (classes 0x4f, 0x55).
                choice @ (CareerChoice::EditBaller | CareerChoice::EditMoves) => {
                    save.story.switch_career(slot);
                    save.store();
                    edit_star(&mut editing, &save.story, &disc, &mut menus, &mut message);
                    if choice == CareerChoice::EditMoves
                        && matches!(menus.top().0, Page::EditBaller(_))
                    {
                        open_moves(&mut menus, true);
                    }
                }
                CareerChoice::Cancel => {}
            }
            return;
        }
        // `FUN_00275148` on the premade picker: Cross asks "CREATE %1S?"
        // (`FUN_00274260`), its rows YES, EDIT, CANCEL, CANCEL lit (as
        // `bal_premade_confirm.png` shows it).
        Page::NewCareer(slot) => {
            menus.push(Page::PremadeAsk(slot), 2);
            return;
        }
        // `FUN_002743d0`: YES makes the career with the premade, EDIT
        // opens the wizard on it (panel 3, `FUN_00274188(screen, 3)`) in a
        // star slot of the career's own, CANCEL goes back to the picker.
        Page::PremadeAsk(slot) if menus.premade_team.is_some() => {
            menus.stack.pop();
            let at = menus.top().1;
            let record = usize::from(street_sim::story::BALLER_RECORDS) + at;
            if cursor <= 1
                && let Some(p) = disc.players.get(record)
            {
                let mut b = ballers::new(slot, p);
                b.first_name = p.first_name.clone();
                b.clothing = (p.clothing & 0x3fff) as u8;
                b.clothing_variant = ((p.clothing as u16) >> 14).min(2) as u8;
                b.wardrobe = b.worn_wardrobe();
                menus.premade_team = None;
                menus.stack.pop();
                if cursor == 0 {
                    // The premade as it is: nothing to pay (`FUN_001e6518`
                    // loads it whole).
                    editing.move_spent = 0;
                    editing.start = None;
                    editing.skill_spend = 0;
                    editing.paid = ballers::price(&b);
                    editing.worn = Some(b.outfit_row());
                    editing.baller = Some(b);
                    match ballers::commit(&mut editing, &mut save.story) {
                        Ok(()) => {
                            save.store();
                            message.0 = "Baller created".into();
                            menus.save_to_card();
                        }
                        Err(e) => message.0 = e,
                    }
                } else {
                    let female = b.female;
                    start_wizard(slot, female, &mut editing, &disc, &mut menus);
                    editing.baller = Some(b);
                    menus.wizard_career = None;
                }
            }
            return;
        }
        Page::PremadeAsk(slot) => {
            menus.stack.pop();
            let at = menus.top().1;
            if cursor == 1 && at < street_sim::story::BALLER_SLOTS {
                let record = usize::from(street_sim::story::BALLER_RECORDS) + at;
                match (career_star_slot(&save.story), disc.players.get(record)) {
                    (Some(star), Some(p)) => {
                        let mut b = ballers::new(star, p);
                        b.first_name = p.first_name.clone();
                        b.wardrobe = b.worn_wardrobe();
                        let female = b.female;
                        start_wizard(star, female, &mut editing, &disc, &mut menus);
                        editing.baller = Some(b);
                        menus.wizard_career = Some(slot);
                    }
                    _ => message.0 = "No room for another baller.".into(),
                }
                return;
            }
            if cursor == 0 && at < street_sim::story::BALLER_SLOTS {
                let record = street_sim::story::BALLER_RECORDS + at as u16;
                save.story.start_career(slot, record);
                // The career's 1,500 less what the premade costs
                // (`FUN_001e6ac8`, then `FUN_001e6518`).
                if let Some(p) = disc.players.get(usize::from(record)) {
                    let mut b = ballers::new(at, p);
                    b.clothing = (p.clothing & 0x3fff) as u8;
                    b.clothing_variant = ((p.clothing as u16) >> 14).min(2) as u8;
                    let left = (i64::from(ballers::START_POINTS) - ballers::preset_charges(&b))
                        .clamp(0, i64::from(u32::MAX)) as u32;
                    save.story.credit_baller(record, 0);
                    let now = save.story.points_for(record);
                    if left >= now {
                        save.story.credit_baller(record, left - now);
                    } else {
                        save.story.spend_baller(record, now - left);
                    }
                }
                save.store();
                editing.baller = None;
                if let Some(top) = menus.stack.last_mut() {
                    *top = (Page::Story, 0);
                }
                // A new career picks its crew first (the original's
                // Create Team in mode 1, class 0x62, before the map).
                begin_crew(&mut menus, &disc, &save.story, session.as_deref());
                menus.push(Page::StoryCrew, 0);
                // "Is This Your Baller?" YES saves the career
                // (`FUN_002796a0`), "Save to memory card?" over Choose
                // Baller.
                menus.save_to_card();
            }
            return;
        }
        Page::Story => {
            // Select City (`FUN_0029f4b0`): the highlighted city's events;
            // a locked one only sounds (sound 6).
            let city = crate::map_view::ROW_CITY[cursor.min(5)];
            if crate::map_view::row_available(&save.story, cursor) {
                menus.stack.push((Page::City(city), 0));
            }
            return;
        }
        Page::City(city) => {
            let rows = story::city_rows(&save.story, city);
            if cursor >= rows.len() {
                return;
            }
            // A locked event says what unlocks it.
            if !save.story.open(rows[cursor]) {
                message.0 = story::event_help(&save.story, &disc, rows[cursor], &|_| None);
                return;
            }
            // The event's intro first.
            let ev = rows[cursor];
            menus.push(Page::EventIntro(ev), 0);
            return;
        }
        Page::EventIntro(ev) => {
            // Continue (`FUN_002be990`, `FUN_002d5d40`, `FUN_002d1a98`).
            // Play Game: a tournament opens its bracket first; a game goes
            // to Choose Players (class 0x1d in mode 5).
            if street_sim::story::EVENTS[ev].kind == street_sim::story::EventKind::Tournament {
                if save.story.crew.len() < CREW_SIZE {
                    message.0 = "Pick a crew of three first".into();
                } else if let Some(top) = menus.stack.last_mut() {
                    *top = (Page::Bracket(ev), 0);
                }
            } else {
                start_event(
                    ev,
                    input.confirmed_by().unwrap_or(Device::Keyboard),
                    &save,
                    &disc,
                    session.as_deref_mut(),
                    &mut setup,
                    &mut running,
                    &mut message,
                );
                if running.0.is_some() {
                    begin_story_players(
                        &mut menus,
                        &disc,
                        &setup,
                        &save.story,
                        crate::choose::Mode::Legend,
                    );
                    menus.push(Page::ChoosePlayers, 0);
                }
            }
            return;
        }
        Page::StoryTeam => {
            let n = save.story.crew.len();
            if cursor >= n {
                menus.stack.pop();
            } else if cursor == 0 {
                message.0 = "Your baller always starts.".into();
            } else if n <= CREW_SIZE {
                message.0 = "Win games to add players to your bench (Update Team).".into();
            } else {
                save.story.toggle_starter(cursor);
                save.store();
            }
            return;
        }
        Page::UpdateTeam(stage) => {
            let Some(mut u) = menus.update.clone() else {
                menus.stack.pop();
                return;
            };
            let crew = &save.story.crew;
            let mut leave = false;
            match stage {
                UpdateStage::Ask => {
                    if cursor == 0 {
                        menus.update_step(UpdateStage::Pick, 0);
                    } else {
                        leave = true;
                    }
                }
                UpdateStage::Pick => match u.beaten.get(cursor) {
                    Some(&p) if crew.contains(&p) => {
                        message.0 = "Already on your team.".into();
                    }
                    Some(&p) => {
                        // The career's list takes the focus on its first
                        // tile (`FUN_002d6c00`).
                        u.pick = Some(p);
                        menus.update_step(UpdateStage::Slot, 0);
                    }
                    // Cancel Update.
                    None => menus.update_step(UpdateStage::Exit, 1),
                },
                UpdateStage::Slot => {
                    // The baller's tile only sounds (6); an empty place
                    // takes the new player after the team; the dialog
                    // opens on No (0x59, `+0xe0`).
                    if cursor != 0 {
                        u.slot = cursor.min(crew.len());
                        menus.update_step(UpdateStage::Accept, 1);
                    }
                }
                UpdateStage::Accept => {
                    if cursor == 0 {
                        if let Some(p) = u.pick
                            && save.story.update_team(u.slot, p)
                        {
                            save.store();
                            message.0 =
                                format!("{} joins your team.", disc.players[usize::from(p)].name());
                            menus.save_to_card();
                        }
                        leave = true;
                    } else {
                        menus.update_step(UpdateStage::Slot, u.slot);
                    }
                }
                UpdateStage::Exit => {
                    if cursor == 0 {
                        leave = true;
                    } else {
                        menus.update_step(UpdateStage::Pick, 0);
                    }
                }
            }
            if leave {
                menus.update = None;
                menus.stack.pop();
            } else {
                menus.update = Some(u);
            }
            return;
        }
        Page::LeagueTeam => {
            // `FUN_0026f958`: Cross stores the focused team for the
            // challenge's side (`FUN_001dbb20(setup, 0, team)`, which in
            // mode 4 writes the user's `+0x4a`; Random draws one of the 29,
            // the rewrite's own roll as Choose Team's) and opens Choose
            // Region (class 0x29); a blank only sounds.
            let roll = save.story.games.wrapping_mul(2_654_435_761) ^ cursor as u32;
            if let Some(team) = crate::chooseteam::pick(
                &crate::chooseteam::league_grid(&disc, &save.story),
                cursor,
                roll,
            ) {
                save.story.league_team = Some(team as u16);
                save.story.sync_league_user();
                save.store();
                *menus.stack.last_mut().unwrap() = (Page::League, 0);
            }
            return;
        }
        Page::League => {
            if cursor < street_sim::story::REGION_MATCHES.len() {
                menus.stack.push((Page::LeagueRegion(cursor), 0));
            }
            return;
        }
        Page::LeagueRegion(r) => {
            let matches = street_sim::story::REGION_MATCHES[r];
            if cursor <= matches.len() {
                if save.story.league_done(r, cursor) {
                    message.0 = "Won. On to the next".into();
                } else {
                    start_league_match(
                        r,
                        cursor,
                        &mut save,
                        &disc,
                        &mut setup,
                        &mut running,
                        &mut message,
                    );
                    team_set(session.as_deref_mut(), &setup, 1);
                    // Choose Players (class 0x1d in mode 4), then the game.
                    if running.0.is_some() {
                        begin_story_players(
                            &mut menus,
                            &disc,
                            &setup,
                            &save.story,
                            crate::choose::Mode::League,
                        );
                        menus.push(Page::ChoosePlayers, 0);
                    }
                }
            }
            return;
        }
        _ => {}
    }

    // Play It's rows are the original's four, each to its arm below.
    let cursor = match page {
        Page::PlayIt => PLAY_IT_ROWS.get(cursor).copied().unwrap_or(usize::MAX),
        _ => cursor,
    };
    match (page, cursor) {
        (Page::Main, 0) => menus.push(Page::PlayIt, 0),
        (Page::Main, 1) => menus.push(Page::MyTeam, 0),
        (Page::Main, 2) => {
            menus.rewards = Default::default();
            menus.push(Page::Rewards, 0);
        }
        (Page::Main, 3) => menus.push(Page::Options, 0),
        (Page::Main, 4) => menus.push(Page::HighScores, crate::score_view::FIRST),
        (Page::Main, 5) => menus.push(Page::Inside, 0),
        (Page::Inside, 0) => menus.push(Page::WhatsNew, 0),
        (Page::Inside, 1) => {
            // `FUN_00293a00`'s screen opens on Dunks - Level 3.
            menus.moves = Default::default();
            menus.push(Page::TheMoves(0), 0);
        }
        (Page::Inside, 2) => menus.push(Page::Bobbito, 0),
        (Page::Inside, 3) => menus.dvd = Some(movie::TRAILER),
        (Page::Inside, 4) => menus.dvd = Some(movie::SSX),
        (Page::WhatsNew, 0) => menus.dvd = Some(movie::WHATS_NEW),
        (Page::WhatsNew, k @ 1..=4) => menus.dvd = Some(&movie::WHATS_NEW[k - 1..k]),
        (Page::Bobbito, k @ 0..=1) => menus.dvd = Some(movie::BOBBITO[k]),
        (Page::Inside | Page::WhatsNew | Page::Bobbito, _) => {
            menus.stack.pop();
        }
        // Choose Sides and User Setup answered their keys above.
        (Page::Sides, _) | (Page::Users, _) => {}
        (Page::Main, _) => {}
        (Page::PlayIt, 0) => {
            menus.league_user_setup = false;
            menus.school_user_setup = false;
            menus.online.hosting = false;
            // Whoever picked Pick Up Game starts on the home side.
            setup.demo = false;
            if setup.seats.is_empty() {
                setup
                    .seats
                    .push((input.confirmed_by().unwrap_or(Device::Keyboard), 0));
            }
            // The original asks the difficulty first (class 105,
            // `FUN_002bf428`), then User Setup.
            menus.push(Page::Difficulty, difficulty_row(setup.rules.difficulty));
        }
        (Page::Difficulty, k) => {
            // `FUN_00284848`: the row's level, then User Setup (class 0xd)
            // in a Pick Up Game, User Setup (0xe) in the League Challenge and
            // the story menu, Choose Baller (0x36), in Be A Legend.
            setup.rules.difficulty = DIFFICULTY_ROWS[k.min(2)];
            let from = menus
                .stack
                .len()
                .checked_sub(2)
                .and_then(|k| menus.stack.get(k))
                .copied();
            match after_difficulty(from) {
                AfterDifficulty::ChooseBaller => menus.push(Page::Careers, 0),
                AfterDifficulty::LeagueUserSetup => {
                    // Choose the owner before restoring or restarting their run.
                    menus.picks.clear();
                    seat_league_user(&mut setup, input.confirmed_by().unwrap_or(Device::Keyboard));
                    menus.league_user_setup = true;
                    menus.school_user_setup = false;
                    begin_names(&mut menus, &mut setup);
                }
                AfterDifficulty::ChooseSides => {
                    // The owner's column opens on the chooser with no side
                    // picked, the icon between AWAY and HOME
                    // (`FUN_002979a8`: state 10; the others state 1).
                    setup.seats.clear();
                    menus.joined = vec![input.confirmed_by().unwrap_or(Device::Keyboard)];
                    menus.push(Page::Sides, 0);
                }
            }
        }
        // The rewrite's own rows, on its PC Settings page (behind Options'
        // Adjust Screen): Online, Watch A Game (the computer on both sides,
        // its teams, players, court and rules picked as a Pick Up game's)
        // and the created teams' list (to edit or delete one).
        (Page::PcSettings, options::pc::ONLINE) => menus.push(Page::Online, 0),
        (Page::PcSettings, options::pc::WATCH) => {
            menus.online.hosting = false;
            setup.seats.clear();
            setup.demo = true;
            push_choose_team(&mut menus, &disc, &save.story, setup.teams[0], 0);
        }
        (Page::PcSettings, options::pc::TEAMS) => menus.push(Page::Teams, 0),
        (Page::PlayIt, 2 | 3) => {
            menus.push(Page::Difficulty, difficulty_row(setup.rules.difficulty));
        }
        (Page::PlayIt, 4) => {
            seat_league_user(&mut setup, input.confirmed_by().unwrap_or(Device::Keyboard));
            menus.league_user_setup = false;
            menus.school_user_setup = true;
            begin_names(&mut menus, &mut setup);
        }
        (Page::ChooseTeam(side), i) => {
            // Random League Team draws one of the 29 (`rand() % 29` in the
            // original; the rewrite's own draw).
            let roll = save.story.games.wrapping_mul(2_654_435_761) ^ i as u32;
            let Some(team) =
                crate::chooseteam::pick(&crate::chooseteam::grid(&disc, &save.story), i, roll)
            else {
                return;
            };
            setup.teams[side] = team;
            computer_three(session.as_deref_mut(), &mut setup, side, &disc, &save.story);
            // Online, this machine's sides only, then their players.
            if let Some(duties) = &menus.online_pick {
                let then = duties
                    .sides
                    .iter()
                    .skip_while(|&&s| s != side)
                    .nth(1)
                    .copied();
                menus.picks.clear();
                match then {
                    Some(other) => {
                        push_choose_team(&mut menus, &disc, &save.story, setup.teams[other], other)
                    }
                    None => {
                        begin_players(&mut menus, &disc, &mut setup, &save.story);
                        menus.push(Page::ChoosePlayers, 0);
                    }
                }
                return;
            }
            // One Choose Team screen for both sides (class 0x1b,
            // `FUN_00270c78`): the person's side ready, the computer's side
            // goes live for the same person (`FUN_00270b28`); with both
            // ready `FUN_0026fd00` goes on to Choose Players (class 0x1d).
            // The rewrite keeps a page per side, the earlier one shown
            // under "Ready".
            menus.picks.clear();
            match after_team(side) {
                (Page::ChooseTeam(other), _) => {
                    push_choose_team(&mut menus, &disc, &save.story, setup.teams[other], other)
                }
                (page, at) => {
                    begin_players(&mut menus, &disc, &mut setup, &save.story);
                    menus.push(page, at);
                }
            }
        }
        (Page::ChooseCourt, i) => {
            // `FUN_002686e0` (0x23): a locked court only sounds (6); an
            // open one is stored (`FUN_001dbc88`) and the Play Game / My
            // Rules dialog opens (`FUN_00269610`).
            let Some(court) = crate::choosecourt::court_of(i, menus.court_random) else {
                return;
            };
            if !crate::choosecourt::open(&save.story, &disc, court) {
                return;
            }
            if let Some(c) = disc.courts.get(court) {
                setup.court = c.id.clone();
            }
            menus.push(Page::PlayGameAsk, 0);
        }
        // `FUN_002696a8`: Play Game loads the match (class 0x2c), My Rules
        // opens Set Challenge (0x27) on the rules as they stand.
        (Page::PlayGameAsk, 0) if menus.online_pick.is_some() => {
            menus.online_picked = true;
        }
        (Page::PlayGameAsk, 0) => next.set(Screen::Loading),
        (Page::PlayGameAsk, _) => {
            menus.rules_before = Some(setup.rules);
            menus.rules_edit = crate::myrules::Edit::from_rules(&setup.rules);
            menus.push(Page::MyRules, 0);
        }
        // `FUN_002c9740` (0x23 on Play Game): the records stored, then the
        // loading class 0x2c; the other rows do nothing.
        (Page::MyRules, crate::myrules::PLAY_GAME) => {
            let edit = menus.rules_edit;
            edit.apply(&mut setup.rules);
            if menus.online_pick.is_some() {
                menus.online_picked = true;
            } else {
                next.set(Screen::Loading);
            }
        }
        (Page::MyRules, _) => {}
        // `FUN_002cb490`: Yes returns to Choose Court (class 0x21) with the
        // rules as they were; No closes the dialog.
        (Page::RulesCancelAsk, 0) => {
            if let Some(before) = menus.rules_before.take() {
                setup.rules = before;
            }
            while matches!(
                menus.top().0,
                Page::RulesCancelAsk | Page::MyRules | Page::PlayGameAsk
            ) && menus.stack.len() > 1
            {
                menus.stack.pop();
            }
        }
        (Page::RulesCancelAsk, _) => {
            menus.stack.pop();
        }
        (Page::Online, k) => match menus.online.row(k) {
            crate::online::Row::Host(people) => {
                // Hosting opens the lobby at once: everyone joins first,
                // then picks (`lobby.rs`).
                menus.online.hosting = true;
                menus.online.people = people;
                setup.seats.clear();
                setup.demo = false;
                menus.go_online = Some(menus.online.args(true));
            }
            crate::online::Row::Lan(i) => menus.go_online = menus.online.args_lan(i),
            crate::online::Row::Code => typing.field = Some(Field::Server),
            crate::online::Row::Name => typing.field = Some(Field::PlayerName),
            crate::online::Row::Spectate => menus.online.step(k, 1),
            crate::online::Row::Browse => {
                if menus.online.server.trim().is_empty() {
                    message.0 = "Type the server's address under Join With Code first.".into();
                } else {
                    match crate::online::MatchBrowser::new(&menus.online.server) {
                        Ok(browser) => {
                            commands.insert_resource(browser);
                            menus.online.answered = false;
                            menus.online.matches.clear();
                            menus.push(Page::MatchBrowser, 0);
                        }
                        Err(why) => message.0 = why,
                    }
                }
            }
        },
        (Page::MatchBrowser, k) => {
            if let Some(m) = menus.online.matches.get(k).cloned() {
                menus.online.match_name = m.name;
                menus.go_online = Some(menus.online.args(false));
            } else {
                menus.stack.pop();
                commands.remove_resource::<crate::online::MatchBrowser>();
            }
        }
        (Page::PcSettings, options::pc::BACK) => {
            keep_options(&mut save, &opts, &sound, &pads, &setup);
            menus.stack.pop();
        }
        (Page::PcSettings, k) => options::pc_step(k, &mut opts.0, &mut pads),
        (Page::Layouts, 0) => menus.controller_player = (menus.controller_player + 1) % 4,
        (Page::Layouts, 1) => {
            let l = &mut opts.0.layouts[menus.controller_player];
            *l = (*l + 1) % 3;
        }
        (Page::Layouts, 2) => menus.controller_view = (menus.controller_view + 1) % 3,
        (Page::Layouts, 3) => pads.vibration = !pads.vibration,
        (Page::Layouts, _) => {
            menus.stack.pop();
            menus.save_to_card();
        }
        // `FUN_00299110`: the card widget's work (`+0x16c` with 0x13f, and
        // `+0x174`), its dialogs over the screen.
        (Page::GameData, 0) => menus.card_work(crate::card::Op::Save),
        (Page::GameData, 1) => menus.card_work(crate::card::Op::Load),
        (Page::GameData, k @ 2..=4) => {
            if count_of(&save, k - 2) == 0 {
                menus.data_ask = Some(crate::game_data::Ask::Nothing(k - 2));
                menus.push(Page::DataAsk, 0);
            } else {
                menus.push(Page::Delete(k - 2), 0);
            }
        }
        // Exit (and Triangle): after a delete the card saves first
        // (`+0xf0`), then Options (0x28).
        (Page::GameData, _) => {
            menus.stack.pop();
            if std::mem::take(&mut menus.data_deleted) {
                menus.save_to_card();
            }
        }
        // A delete list's entry: the question in the list's place.
        (Page::Delete(kind), row) if row < count_of(&save, kind) => {
            let ask = match kind {
                2 => {
                    let record = save.story.ballers[row].record() as u16;
                    let teams: Vec<String> = save
                        .story
                        .teams
                        .iter()
                        .filter(|t| t.roster.contains(&record))
                        .map(|t| t.name.clone())
                        .collect();
                    let b = &save.story.ballers[row];
                    let name = format!("{} {}", b.first_name, b.last_name)
                        .trim()
                        .to_owned();
                    if teams.is_empty() {
                        crate::game_data::Ask::Sure { kind, row, name }
                    } else {
                        crate::game_data::Ask::OnTeam { row, name, teams }
                    }
                }
                0 => crate::game_data::Ask::Sure {
                    kind,
                    row,
                    name: save.story.records.users[row].name.clone(),
                },
                _ => crate::game_data::Ask::Sure {
                    kind,
                    row,
                    name: save.story.teams[row].name.clone(),
                },
            };
            let first = ask.first_row();
            menus.data_ask = Some(ask);
            if let Some(top) = menus.stack.last_mut() {
                *top = (Page::DataAsk, first);
            }
        }
        (Page::Delete(_), _) => {
            menus.stack.pop();
        }
        // Yes deletes (a baller's teams first), then "Delete successful.";
        // No and OK close.
        (Page::DataAsk, row) => {
            let ask = menus.data_ask.take();
            menus.stack.pop();
            let (kind, at, teams) = match ask {
                Some(crate::game_data::Ask::Sure { kind, row: at, .. }) if row == 0 => {
                    (kind, at, Vec::new())
                }
                Some(crate::game_data::Ask::OnTeam { row: at, teams, .. }) if row == 0 => {
                    (2, at, teams)
                }
                _ => return,
            };
            for name in teams {
                if let Some(t) = save.story.teams.iter().position(|t| t.name == name) {
                    delete_entry(1, t, &mut save, &mut team_editing);
                }
            }
            let (_, restore) = delete_entry(kind, at, &mut save, &mut team_editing);
            menus.restore = restore;
            menus.data_deleted = true;
            save.store();
            // "Delete successful." (`FUN_002278c0` case 10), timed.
            menus.card_work(crate::card::Op::Deleted);
        }
        (Page::Controls, _) => {
            menus.stack.pop();
        }
        _ => {}
    }
}

/// The options as they stand, into the game data (the card's next save
/// writes them: leaving Options saves when they changed).
/// The question Select opens on `page` for `device` (`None`: whichever
/// asked), or `None` where Select does nothing: a dialog over the screen
/// takes the press, the rewrite's own pages ask nothing, a decided
/// bracket nothing (its `+0x10c`), User Setup, Choose Players, Choose
/// Court and My Rules only a controller with a person in the game
/// (`FUN_001dbda0`'s `+0xd8`, which a pad gets as it confirms its way in),
/// and Be A Legend's map, an undecided bracket and Choose Region only the
/// user's own pad (the game data's `+0xd4`, the pad seated for the run).
fn select_asks(
    page: Page,
    menus: &Menus,
    story: &street_sim::story::Story,
    seats: &[(Device, usize)],
    device: Option<Device>,
) -> Option<crate::exit_ask::Asked> {
    if menus.under_dialog().is_some() || page == Page::PcSettings {
        return None;
    }
    if matches!(page, Page::Bracket(ev) if tournament_over(story, ev)) {
        return None;
    }
    let class = screen_of(page, menus, story)?;
    let seated = matches!(class, 0x0c..=0x13 | 0x1d | 0x2b | 0x21 | 0x27);
    if let Some(device) = device
        && seated
        && !seats.is_empty()
        && !seats.iter().any(|&(d, _)| d == device)
    {
        return None;
    }
    let users_own = matches!(class, 0x38 | 0x58 | 0x59 | 0x5e | 0x29);
    if let Some(device) = device
        && users_own
        && seats.first().is_some_and(|&(user, _)| user != device)
    {
        return None;
    }
    crate::exit_ask::asked(class)
}

/// Select's question answered Yes: back to Main (`FUN_002d7d40(.., 3)`,
/// the pages over it gone), with what the screen does besides
/// ([`crate::exit_ask::Yes`]). The screens' leaving saves are not made:
/// Game Data's after a delete, Rewards' after a purchase, Options' and the publisher
/// Trax's, the editor's.
fn exit_to_main(
    asked: crate::exit_ask::Asked,
    menus: &mut Menus,
    save: &mut story::Save,
    (opts, sound, difficulty): (
        &mut options::Options,
        &mut AudioSettings,
        &mut street_sim::Difficulty,
    ),
    editing: &mut ballers::Editing,
    (running, cheats): (&mut story::Running, &mut crate::cheats::Settings),
) {
    use crate::exit_ask::Yes;
    match asked.yes {
        Yes::Main | Yes::KeepTrax => {}
        Yes::CancelOptions => {
            if let Some(before) = menus.options_before {
                put_back_options(before, opts, sound, difficulty);
            }
        }
        Yes::ClearCodes => cheats.reset(),
        Yes::KeepBaller => {
            if editing.baller.is_some() && ballers::commit(editing, &mut save.story).is_ok() {
                save.store();
            }
        }
    }
    // What the pages over Main held: a game made ready (Be A Legend's, the
    // League Challenge's), a baller being made, the screens' leaving saves.
    running.0 = None;
    editing.baller = None;
    editing.start = None;
    menus.wizard_career = None;
    menus.premade_team = None;
    menus.cab_ask = None;
    menus.moves.preview = None;
    menus.options_before = None;
    menus.options_top = 0;
    menus.trax_before = None;
    menus.trax_top = 0;
    menus.data_deleted = false;
    menus.rewards_bought = false;
    menus.hold_ms = crate::exit_ask::hold_ms(asked.class);
    menus.whoosh = Some(crate::exit_ask::whooshes(asked.class));
    match menus.stack.iter().position(|&(p, _)| p == Page::Main) {
        Some(k) => menus.stack.truncate(k + 1),
        None => menus.stack = vec![(Page::Title, 0), (Page::Main, 0)],
    }
}

/// Options' settings as the screen opened on them (its cancel question's
/// Yes, and Select's, `FUN_002a6d40`); the rewrite's own ones are not the
/// screen's to put back.
fn put_back_options(
    before: street_sim::records::Settings,
    opts: &mut options::Options,
    sound: &mut AudioSettings,
    difficulty: &mut street_sim::Difficulty,
) {
    opts.0 = street_sim::records::Settings {
        vibration: opts.0.vibration,
        full_screen: opts.0.full_screen,
        wide: opts.0.wide,
        ..before
    };
    sound.music = before.music;
    sound.effects = before.effects;
    sound.speech = before.speech;
    *difficulty = street_sim::Difficulty::from_level(before.difficulty);
}

fn keep_options(
    save: &mut story::Save,
    opts: &options::Options,
    sound: &AudioSettings,
    pads: &crate::pads::PadSettings,
    setup: &MatchSetup,
) {
    save.story.settings = Some(options::gather(
        &opts.0,
        sound,
        pads,
        setup.rules.difficulty,
    ));
    save.store();
}

/// Leaving a decided tournament's bracket: Update Team when the title
/// offered a player (it saves on its accept, `FUN_002d7238`), else the
/// city's page through the card's save.
fn leave_bracket(menus: &mut Menus) {
    if menus.update.is_some() {
        if let Some(top) = menus.stack.last_mut() {
            *top = (Page::UpdateTeam(UpdateStage::Ask), 0);
        }
    } else {
        menus.stack.pop();
        menus.save_to_card();
    }
}

/// Whether a tournament's bracket is decided (won, or out of it).
fn tournament_over(story: &street_sim::story::Story, ev: usize) -> bool {
    story
        .brackets
        .iter()
        .any(|(e, b)| usize::from(*e) == ev && (b.won() || b.out()))
}

/// Deletes row `row` of delete list `kind`; what to tell the player.
fn delete_entry(
    kind: usize,
    row: usize,
    save: &mut story::Save,
    team_editing: &mut teams::TeamEditing,
) -> (String, Option<usize>) {
    let done = || "Delete successful.".to_owned();
    match kind {
        0 => {
            let active =
                save.story.records.users.get(row).is_some_and(|user| {
                    save.story.league_user.as_deref() == Some(user.name.as_str())
                });
            let school_active =
                save.story.records.users.get(row).is_some_and(|user| {
                    save.story.school_user.as_deref() == Some(user.name.as_str())
                });
            save.story.records.delete_user(row);
            if school_active {
                save.story.select_school_user(None);
            }
            if active {
                save.story.select_league_user(None);
            }
            (done(), None)
        }
        1 => {
            save.story.teams.remove(row);
            team_editing.dirty = true;
            (done(), None)
        }
        _ => {
            let b = save.story.ballers[row].clone();
            let record = b.record() as u16;
            // `delete.txt` 18: the team must go first.
            if let Some(t) = save.story.teams.iter().find(|t| t.roster.contains(&record)) {
                return (
                    format!(
                        "{} {} is on the Created Team {} {}. The Created Team must be deleted first.",
                        b.first_name, b.last_name, t.city, t.name
                    ),
                    None,
                );
            }
            save.story.ballers.remove(row);
            // Its place on a league team goes with it.
            save.story.leave_team(record);
            (done(), Some(b.record()))
        }
    }
}

/// How many entries a delete list holds (0 names, 1 created teams, 2
/// created ballers).
fn count_of(save: &story::Save, kind: usize) -> usize {
    match kind {
        0 => save.story.records.users.len(),
        1 => save.story.teams.len(),
        _ => save.story.ballers.len(),
    }
}

/// A deleted baller's record back as the disc has it.
fn restore_records(mut menus: ResMut<Menus>, mut disc: ResMut<Disc>, save: Res<story::Save>) {
    if let Some(record) = menus.restore.take() {
        ballers::restore(&mut disc, record);
        // A league team it had joined has five again.
        teams::apply_all(&mut disc, &save.story);
    }
    if std::mem::take(&mut menus.teams_changed) {
        teams::apply_all(&mut disc, &save.story);
    }
}

/// A name typed on the name page: added to the users and given to the
/// seat (or the reason it can't be).
fn commit_name(
    typing: Res<Typing>,
    mut menus: ResMut<Menus>,
    mut save: ResMut<story::Save>,
    mut setup: ResMut<MatchSetup>,
    mut message: ResMut<Message>,
    look: Option<Res<Look>>,
) {
    if !typing.ended {
        return;
    }
    let Some(k) = menus.naming.take() else {
        return;
    };
    let mut name = std::mem::take(&mut menus.new_name);
    // DONE over the keyboard's default keeps it ("PLAYER%1d").
    if name.trim().is_empty() {
        name = crate::users::default_name(look.as_deref(), k);
    }
    let stage = match crate::highscores::name_seat(&mut save.story, &mut setup, k, &name) {
        Ok(()) => {
            save.store();
            message.0 = format!("Welcome, {}", name.trim());
            UserStage::Named
        }
        // `entername.txt` 32, 38: the column asks again.
        Err(why) => {
            message.0 = why.into();
            UserStage::START
        }
    };
    if let Some(s) = menus.user_stages.get_mut(k) {
        *s = stage;
    }
}

/// The options saved last time, or the original's defaults on a fresh
/// save, applied at start. The match's difficulty is `main`'s: the saved
/// option unless `--difficulty` chose one.
fn apply_settings(
    save: Res<story::Save>,
    mut sound: ResMut<AudioSettings>,
    mut sounds: ResMut<crate::audio::Sounds>,
    mut pads: ResMut<crate::pads::PadSettings>,
    mut opts: ResMut<options::Options>,
) {
    let s = save.story.settings.unwrap_or_default();
    sounds.trax(s.songs_off, s.sequential, s.interactive);
    opts.0 = s;
    sound.music = s.music;
    sound.effects = s.effects;
    sound.speech = s.speech;
    pads.vibration = s.vibration;
}

#[derive(Resource)]
struct ReloadData;

/// Game Data's Load, at the card's Loading end: the file's game data in
/// place of the active one, the models reset from the user's disc before
/// its edits. A file that does not read back leaves the data as it is (the
/// card's "Load failed." says so).
fn reload_data(world: &mut World) {
    if world.remove_resource::<ReloadData>().is_none() {
        return;
    }
    let path = world.resource::<story::Save>().path.clone();
    let result = std::fs::read(&path)
        .map_err(|e| e.to_string())
        .and_then(|bytes| {
            street_sim::story::Story::read(&bytes)
                .ok_or_else(|| "The save file is damaged or unsupported.".into())
        });
    let Ok(mut data) = result else {
        return;
    };
    let mut disc = world.resource_mut::<Disc>();
    let loaded = match street_assets::db::players(&mut disc.iso) {
        Ok(players) => {
            data.sanitize(players.len(), disc.base_teams, disc.moves.len());
            data.trophy_rewards();
            disc.players = players;
            ballers::apply_all(&mut disc, &data);
            teams::apply_all(&mut disc, &data);
            Ok(())
        }
        Err(e) => Err(format!("Could not load players: {e}")),
    };
    if let Err(why) = loaded {
        world.resource_mut::<Message>().0 = why;
        return;
    }
    let settings = data.settings.unwrap_or_default();
    world.resource_mut::<story::Save>().story = data;
    world.resource_mut::<options::Options>().0 = settings;
    {
        let mut sound = world.resource_mut::<AudioSettings>();
        sound.music = settings.music;
        sound.effects = settings.effects;
        sound.speech = settings.speech;
    }
    world.resource_mut::<crate::pads::PadSettings>().vibration = settings.vibration;
    {
        let mut setup = world.resource_mut::<MatchSetup>();
        setup.rules.difficulty = street_sim::Difficulty::from_level(settings.difficulty);
        setup.teams = [0, 1];
    }
    *world.resource_mut::<ballers::Editing>() = Default::default();
    *world.resource_mut::<teams::TeamEditing>() = Default::default();
}

/// The memory card's dialogs own the keys while it works (`FUN_00259380`'s
/// list: up and down between its rows, Cross on one), its timed messages
/// move on by themselves, and its work is done on the file: written at
/// Saving's end, read back at Loading's. Once done, the pages the save
/// point went on to show come.
fn card_menu(
    input: MenuInput,
    mut card: ResMut<crate::card::Card>,
    mut menus: ResMut<Menus>,
    mut save: ResMut<story::Save>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    (opts, sound, pads, setup): (
        Res<options::Options>,
        Res<AudioSettings>,
        Res<crate::pads::PadSettings>,
        Res<MatchSetup>,
    ),
) {
    use crate::card::Work;
    card.closed = false;
    let Some(flow) = card.flow.as_mut() else {
        return;
    };
    crate::card::keys(flow, &input, f64::from(time.delta_secs()));
    match flow.work() {
        Some(Work::Write) => {
            // The game data holds the settings as they stand.
            save.story.settings = Some(options::gather(
                &opts.0,
                &sound,
                &pads,
                setup.rules.difficulty,
            ));
            crate::card::write(flow, &mut save);
        }
        Some(Work::Read) => {
            let ok = crate::card::Holds::of(&save.path) == crate::card::Holds::Data;
            if ok {
                commands.insert_resource(ReloadData);
            }
            flow.read(ok);
        }
        Some(Work::Reset) => {
            save.story = Default::default();
            save.story.trophy_rewards();
        }
        None => {}
    }
    if flow.done {
        let saved = flow.saved;
        card.flow = None;
        card.closed = true;
        if let Some(stack) = menus.after_card.take() {
            // My Team's Create Team stays on its grid when the card's work
            // ends unsaved (the widget's `+0xb0` clear, `FUN_002808c0`).
            if std::mem::take(&mut menus.team_saving) && !saved {
                team_card_closed(&mut menus);
            } else {
                menus.stack = stack;
            }
        }
        // Lists left under the card start at their tops next time.
        if !menus.stack.iter().any(|(p, _)| *p == Page::Options) {
            menus.options_top = 0;
        }
        if !menus.stack.iter().any(|(p, _)| *p == Page::Trax) {
            menus.trax_top = 0;
        }
    }
}

/// A save point's request: the card's work begins over the pages the keys
/// found, the pages it went on to kept for after.
fn card_hold(
    mut card: ResMut<crate::card::Card>,
    mut menus: ResMut<Menus>,
    save: Res<story::Save>,
    opts: Res<options::Options>,
) {
    let Some((op, under)) = menus.card_request.take() else {
        return;
    };
    let holds = crate::card::Holds::of(&save.path);
    card.flow = Some(crate::card::Flow::new(op, holds, opts.0.autosave));
    let after = std::mem::replace(&mut menus.stack, under);
    menus.after_card = Some(after);
}

/// Whether a Screen page's row can take the cursor when the navigation
/// graph lands on it: `FUN_00222770` on the kind-10 node `FUN_0021f8f0`
/// reaches, which needs a bound display widget (node +0xb0) whose byte
/// +0x5b has bit 1, the enabled flag, set.
///
/// The widget half: `FUN_0021f230` pairs the list's nodes with its
/// registered widgets in order, so a node past the page's `rows` has none
/// and is skipped (What's New and Bobbito author more nodes than rows).
///
/// The enabled half: `FUN_002ddb40` enables the incoming screen and all its
/// children (`+0x10c` = `FUN_002490f8`, recursive, sets bit 1), and the
/// Screen pages here (classes 3, 5, 6, 102, 103, 104 and the map, 56)
/// never disable a single row: their only `+0x114` (`FUN_002491d8`) calls
/// switch off the whole list on the way out (`FUN_0029e608`,
/// `FUN_002bf320`, `FUN_002a3fa8`, `FUN_00289e68`, `FUN_0028adb0`,
/// `FUN_00288fd0`) or toggle the map's panels (`FUN_002a14f8`), and none
/// sets the sticky bit 3 that would keep a row off. The map's locked
/// events stay selectable, only dimmed (`FUN_00224e00`), and refuse a
/// confirm with sound 6 (`FUN_0029f4b0`), as [`Page::City`] does. So every
/// bound row is available whatever the save holds.
fn screen_row_available(row: usize, rows: usize) -> bool {
    row < rows
}

/// Play It's arms below by row: Pick Up Game, League Challenge, Be A Legend
/// and Street School (`playit.txt` 2, 3, 4, 1 on the authored four).
const PLAY_IT_ROWS: [usize; 4] = [0, 2, 3, 4];
/// Be A Legend's row on Play It.
const PLAY_IT_LEGEND: usize = 2;
/// Street School's row on Play It.
const PLAY_IT_SCHOOL: usize = 3;

/// A screen's loop: the movie its slot `+0x15c` names and the pair it
/// gives, its top-left corner in the layouts' 640x480 (`FUN_002ddb40`
/// subtracts 16 from y for the 448-line frame and places the movie widget
/// by its corner at its own size, unscaled), and that size.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScreenLoop {
    pub path: &'static str,
    pub at: Vec2,
    pub size: Vec2,
}

impl ScreenLoop {
    pub fn rect(&self) -> Rect {
        Rect::from_corners(self.at, self.at + self.size)
    }
}

/// The loop a screen of (factory) class `class` plays over its backdrop,
/// by its slot `+0x15c` (the base `FUN_002d8fb0` names none):
///
/// - the main backdrop's screens, the city in the right window: Main
///   (`FUN_0029f018`), Play It (`FUN_002bfb08`), My Team (`FUN_002a45f0`),
///   Difficulty (`FUN_00284f78`), Inside, What's New and Bobbito
///   (`FUN_0028a6f0`, `FUN_0028b6a0`, `FUN_002897b0`): `xlp0005` at (265,
///   240);
/// - User Setup (`FUN_00297848`'s screens, `0x298e20`): `xlp0004` at (464,
///   277), the cars passing low on the right;
/// - Choose Team (`FUN_0026fb00`'s, classes 0x1b and 0x1c, `0x272f38`):
///   `xlp0003` at (0, 0), the trees over the top left; the League Challenge's
///   (0x19, `FUN_0026f4f0`, `0x272e90`): `xlp0011` there;
/// - Choose Players and the League Challenge's (0x1d, 0x2b, `0x26b330`):
///   `xlp0002` at (480, 1), the right column's smoke and water tower;
/// - Choose Court (0x21, `0x2699f0`): `xlp0001` at (19, 0); My Rules (0x27,
///   `0x2cbe08`): `xlp0010` at (17, 0), the smoke over the top left;
/// - the League Challenge's completion board (0x3b, `FUN_002a4bc8`): `xlp0006`
///   at (100, 146), the trophy turning in the left panel; Be A Legend's
///   (0x3c, `0x262498`): `xlp0016` at (69, 141), its trophy.
///
/// Rewards (`FUN_002c55e8`, `xlp0009`) moves and swaps its own
/// (`FUN_002c3948`, [`crate::rewards_view`]). In the project's PCSX2
/// runtime the front end's loop widget (`+0x58`) held each of these files
/// on its screen (2026-10-20; the boards reached by the front end's
/// request), and the grabs put User Setup's, Choose Players', the boards'
/// at their pairs (the grabs' own offset of 1 to 3 pixels aside).
pub fn screen_loop(class: u16) -> Option<ScreenLoop> {
    let (path, x, y, w, h) = match class {
        0x03 | 0x05 | 0x06 | 0x66..=0x69 => ("/MOVIES/XLP0005.NFX", 265, 240, 320, 240),
        0x0c..=0x13 => ("/MOVIES/XLP0004.NFX", 464, 277, 176, 96),
        0x19 => ("/MOVIES/XLP0011.NFX", 0, 0, 336, 192),
        0x1b | 0x1c => ("/MOVIES/XLP0003.NFX", 0, 0, 336, 192),
        0x1d | 0x2b => ("/MOVIES/XLP0002.NFX", 480, 1, 160, 416),
        0x21 => ("/MOVIES/XLP0001.NFX", 19, 0, 176, 288),
        0x27 => ("/MOVIES/XLP0010.NFX", 17, 0, 176, 288),
        0x3b => ("/MOVIES/XLP0006.NFX", 100, 146, 160, 208),
        0x3c => ("/MOVIES/XLP0016.NFX", 69, 141, 192, 208),
        _ => return None,
    };
    Some(ScreenLoop {
        path,
        at: Vec2::new(x as f32, y as f32),
        size: Vec2::new(w as f32, h as f32),
    })
}

/// The loop a page's screen plays (`class`: its [`screen_of`]). Its
/// screens answer "sameloop" for the screens before them that ran it too
/// ([`crate::transition::sameloop`]), and the film room keeps it running
/// then; any other change makes it anew.
pub fn loop_movie_for_page(page: &Page, class: Option<u16>) -> Option<String> {
    // The Rewards' movies are its own (`moves_panel`).
    if matches!(
        page,
        Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_)
    ) {
        return None;
    }
    class.and_then(screen_loop).map(|l| l.path.to_owned())
}

/// Whether the page's movie sits between the backdrop and the screen's
/// widgets: a screen's loop repaints part of its opaque backdrop, under
/// the rows that overlap it (Main Menu rows at y 230..362); the film
/// room's clips draw on top.
pub fn movie_under_rows(page: &Page, class: Option<u16>) -> bool {
    loop_movie_for_page(page, class).is_some()
}

/// Where the film room's picture goes on a page: its screen's loop's
/// place, or the film room's own panel.
pub fn movie_panel_rect(page: &Page, class: Option<u16>) -> Option<Rect> {
    match page {
        // The film room's movie widget (`FUN_00292ce0`; The Moves' the same).
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) | Page::TheMoves(_) => {
            Some(crate::edit_moves::FILM_ROOM)
        }
        _ if loop_movie_for_page(page, class).is_some() => {
            class.and_then(screen_loop).map(|l| l.rect())
        }
        _ => None,
    }
}

/// The moves page's film room: the highlighted move's clip, in the panel
/// the baller preview gives up meanwhile.
fn moves_panel(
    menus: Res<Menus>,
    disc: Res<Disc>,
    save: Res<story::Save>,
    mut film: ResMut<crate::movie::FilmRoom>,
    mut preview: ResMut<ballers::Preview>,
) {
    let (page, _) = menus.top();
    let want = match page {
        // Edit Moves and The Moves play a clip only when asked ("Preview
        // Move"); the backdrop's still shows meanwhile.
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) | Page::TheMoves(_) => menus
            .moves
            .preview
            .and_then(|row| disc.moves.get(usize::from(row)))
            .map(|m| crate::movie::FilmRoom::path(m.move_id)),
        // Rewards: the column's trophies (`XLP0009`) once one is earned;
        // on the Trophies tab an earned trophy's own movie.
        Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_) => {
            let s = &menus.rewards;
            let won = save.story.trophies();
            if s.tab == crate::rewards_view::TROPHIES {
                won[s.trophy.min(3)]
                    .then(|| crate::rewards_view::TROPHY_MOVIES[s.trophy.min(3)].to_owned())
            } else {
                won.iter()
                    .any(|&w| w)
                    .then(|| "/MOVIES/XLP0009.NFX".to_owned())
            }
        }
        _ => loop_movie_for_page(&page, screen_of(page, &menus, &save.story)),
    };
    if film.want != want {
        film.want = want;
    }
    let hidden = matches!(
        page,
        Page::MovePick(..) | Page::MoveSlots(_) | Page::MoveAsk(..)
    );
    if preview.hidden != hidden {
        preview.hidden = hidden;
    }
}

/// Moves a controller one step between Away (-1), CPU (0) and Home (+1).
/// Joining takes the next number (1P..4P) while there is one free.
fn move_seat(seats: &mut Vec<(Device, usize)>, device: Device, step: i32) {
    let at = seats.iter().position(|&(d, _)| d == device);
    let now = at.map_or(0, |k| if seats[k].1 == 0 { 1 } else { -1 });
    let next = (now + step).clamp(-1, 1);
    match (at, next) {
        (Some(k), 0) => {
            seats.remove(k);
        }
        (Some(k), side) => seats[k].1 = usize::from(side < 0),
        (None, 0) => {}
        (None, side) => {
            if seats.len() < MAX_SEATS {
                seats.push((device, usize::from(side < 0)));
            }
        }
    }
}

/// `FUN_001dbb20` for `side`: a team set for a side no person holds
/// reseeds the session's generators (`session.rs`).
fn team_set(session: Option<&mut crate::session::Session>, setup: &MatchSetup, side: usize) {
    if let Some(session) = session {
        session.team_set(setup, side);
    }
}

/// Choose Team's commit for `side` (`FUN_001dbb20`): with no person on it
/// the session is reseeded and `FUN_001dbe30` draws its three in Pick Up's
/// mode ([`crate::lineup`]), apart from the other side's when no person
/// holds that one either (the line-ups start empty, `FUN_001dec28`).
pub(crate) fn computer_three(
    session: Option<&mut crate::session::Session>,
    setup: &mut MatchSetup,
    side: usize,
    disc: &Disc,
    story: &street_sim::story::Story,
) {
    let mut local = crate::session::Session::pinned(setup.seed);
    let session = session.unwrap_or(&mut local);
    session.team_set(setup, side);
    if crate::session::held(setup, side) {
        return;
    }
    let other = if crate::session::held(setup, 1 - side) {
        [None; 3]
    } else {
        setup.lineups[1 - side].map(Some)
    };
    let book = crate::lineup::Book::of(disc);
    let unlocked = crate::lineup::unlocked(disc, story);
    let mode = crate::lineup::Mode::PickUp {
        unlocked: &unlocked,
    };
    setup.lineups[side] = session.choose(&book, &mode, setup.teams[side], other, None);
}

/// Start a story event's match, if the crew is seated and the event is open.
#[allow(clippy::too_many_arguments)]
fn start_event(
    ev: usize,
    device: Device,
    save: &story::Save,
    disc: &Disc,
    session: Option<&mut crate::session::Session>,
    setup: &mut MatchSetup,
    running: &mut story::Running,
    message: &mut Message,
) {
    if save.story.crew.len() < CREW_SIZE {
        message.0 = "Pick a crew of three first".into();
        return;
    }
    if !save.story.open(ev) {
        message.0 = "Locked. Earn more progress points first".into();
        return;
    }
    // The career's person, on the career's side (`story::CAREER_SIDE`).
    seat_league_user(setup, device);
    let mut local = crate::session::Session::pinned(setup.seed);
    let session = session.unwrap_or(&mut local);
    match story::prepare(ev, &save.story, disc, setup, session) {
        Ok(()) => running.0 = Some(story::Run::StoryEvent(ev)),
        Err(e) => message.0 = e,
    }
}

/// Start one League Challenge match.
#[allow(clippy::too_many_arguments)]
fn start_league_match(
    region: usize,
    index: usize,
    save: &mut story::Save,
    disc: &Disc,
    setup: &mut MatchSetup,
    running: &mut story::Running,
    message: &mut Message,
) {
    setup.demo = setup.watch;
    save.story.start_league_region(region);
    match story::prepare_league(region, index, &save.story, disc, setup) {
        Ok(()) => running.0 = Some(story::Run::League(region, index)),
        Err(e) => message.0 = e,
    }
}

/// League Challenge and Be A Legend runs belong to the person who entered the
/// mode. Retain that controller's name, and seat only that selected person
/// on the side those modes give the person's team (`story::CAREER_SIDE`;
/// the School moves the seat to its own side 1 too).
fn seat_league_user(setup: &mut MatchSetup, device: Device) {
    let name = setup
        .seats
        .iter()
        .position(|&(d, _)| d == device)
        .and_then(|index| setup.users.get(index))
        .cloned()
        .flatten();
    setup.seats = vec![(device, story::CAREER_SIDE)];
    setup.users = vec![name];
    setup.online = false;
    setup.demo = setup.watch;
}

/// Start a Street School lesson's practice game.
#[allow(clippy::too_many_arguments)]
/// The screen class a page shows, for the Help overlay's table (as `draw`
/// styles the pages; User Setup's side chooser is class 0xb, its names
/// 0xc, the League Challenge's Choose Team 0x19). A screen with panels asks
/// with the focused panel's class (`FUN_00250000`'s fourth argument,
/// `+0x40` of the panel: `FUN_0028ce30` for the editor): the wizard's
/// (`Step::layout`, 63..73), the editor's (80..84), Edit Moves' steps (85,
/// 86), The Moves' (101); Rewards' close-up asks 21, 22 or 23 by the
/// card (`Display Reward Info` on a legend's, `Use Development Points` on
/// the development points'), its Trophies tab 24.
fn screen_class(
    page: Page,
    menus: &Menus,
    story: &street_sim::story::Story,
    disc: &Disc,
) -> Option<u32> {
    Some(match page {
        Page::Main | Page::Online => 3,
        Page::PlayIt => 5,
        Page::Difficulty => 105,
        Page::Sides => 11,
        Page::Users => 12,
        Page::ChooseTeam(_) => 26,
        Page::LeagueTeam => 25,
        Page::NewCareer(_) => 63,
        Page::Wizard(_, step) => crate::cab_view::Step::at(step).layout(),
        Page::EditBaller(_) => crate::cab_view::Edit::at(menus.edit_panel).layout(),
        Page::MoveSlots(_) => 85,
        Page::MovePick(..) => 86,
        Page::TheMoves(_) => 101,
        Page::ChoosePlayers => crate::choose::CLASS,
        Page::League | Page::LeagueRegion(_) => crate::region_view::CLASS,
        Page::ChooseCourt => 33,
        Page::MyRules => 39,
        Page::MyTeam => 6,
        Page::Inside => 102,
        Page::WhatsNew => 103,
        Page::Bobbito => 104,
        Page::Story => crate::map_view::CLASS,
        Page::City(_) => 57,
        Page::EventIntro(ev) => crate::event_intro::class(ev),
        Page::Bracket(ev) => {
            crate::bracket_view::class_of(crate::bracket_view::shown(story, ev).kind)
        }
        Page::StoryCrew | Page::JoinTeam => 77,
        Page::UpdateTeam(_) => 96,
        Page::Careers => crate::careers::CLASS,
        Page::Ballers => 61,
        Page::CreateTeam | Page::EditTeam(_) => crate::create_team::CLASS,
        Page::Rewards => match menus.rewards.view {
            Some(k) => match crate::rewards_view::viewable(story, disc).get(k) {
                Some(c) if c.category <= 1 => 22,
                Some(c) if c.category == 4 => 23,
                _ => 21,
            },
            None if menus.rewards.tab == crate::rewards_view::TROPHIES => 24,
            None => crate::rewards_view::CLASS,
        },
        Page::Options => crate::options_view::CLASS,
        Page::Layouts => 55,
        Page::Trax => 48,
        Page::GameData => crate::game_data::CLASS,
        Page::HighScores => crate::score_view::CLASS,
        _ => return None,
    })
}

/// The original's screen a page is, by its factory class
/// (`FUN_002e55c0`), for the screens' transitions
/// ([`crate::transition`]); a dialog is its screen's. None for the
/// rewrite's own pages, which change without one.
pub(crate) fn screen_of(
    page: Page,
    menus: &Menus,
    story: &street_sim::story::Story,
) -> Option<u16> {
    Some(match page {
        // The press-Start screen is the attract's (class 99), whose Start
        // goes to Main (`FUN_002d38b0`'s screen).
        Page::Title => 0x63,
        Page::Main => 0x03,
        Page::PlayIt => 0x05,
        Page::MyTeam => 0x06,
        Page::Difficulty => 0x69,
        // User Setup: Pick Up's (0xd), the League Challenge's (0xe) and the
        // School's (0xc) share their slots.
        Page::Sides | Page::Users => 0x0d,
        Page::ChooseTeam(_) => 0x1b,
        Page::LeagueTeam => 0x19,
        // The League Challenge's Choose Players is its own class on the same
        // code (`FUN_0026afc8` over `FUN_00269bb8`).
        Page::ChoosePlayers if menus.players.mode == crate::choose::Mode::League => 0x2b,
        Page::ChoosePlayers => 0x1d,
        Page::ChooseCourt | Page::PlayGameAsk => 0x21,
        Page::MyRules | Page::RulesCancelAsk => 0x27,
        Page::Careers | Page::CareerAsk(_) => 0x36,
        Page::Ballers | Page::BallerAsk(_) => 0x3d,
        Page::NewCareer(_) | Page::PremadeAsk(_) => 0x4c,
        Page::Wizard(..) | Page::WizardAsk(_) | Page::CabAsk(_) => 0x4a,
        Page::EditBaller(_) => 0x4f,
        Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) => 0x55,
        Page::TheMoves(_) => 0x65,
        Page::Story | Page::City(_) => 0x38,
        Page::EventIntro(ev) => crate::event_intro::class(ev) as u16,
        Page::Bracket(ev) => {
            crate::bracket_view::class_of(crate::bracket_view::shown(story, ev).kind) as u16
        }
        Page::StoryCrew => 0x62,
        Page::CreateTeam | Page::Teams | Page::EditTeam(_) | Page::PickPlayer => 0x35,
        Page::UpdateTeam(_) => 0x60,
        Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_) => 0x14,
        Page::Completed(mode) => crate::completed_view::class_of(mode) as u16,
        Page::JoinTeam => 0x4d,
        Page::League | Page::LeagueRegion(_) => 0x29,
        Page::HighScores => 0x0a,
        Page::Options | Page::OptionsAsk | Page::PcSettings => 0x28,
        Page::Trax | Page::TraxAsk => 0x30,
        Page::Layouts => 0x37,
        Page::GameData | Page::Delete(_) | Page::DataAsk => 0x61,
        Page::Inside => 0x66,
        Page::WhatsNew => 0x67,
        Page::Bobbito => 0x68,
        Page::Credits => crate::credits::INTRO_CLASS,
        Page::CreditsRoll(team) => team.class(),
        Page::School | Page::StoryTeam | Page::Online | Page::MatchBrowser | Page::Controls => {
            return None;
        }
    })
}

/// The screen the menus open on after the load back (as `spawn_menu`
/// builds its pages), and whether the load back goes there forward: to
/// Update Team and the completion boards it does (`FUN_002d7cb0`), to the
/// rest back (`FUN_002d7d40`; the load back's calls, `0x29d368` on).
pub(crate) fn arrival(
    menus: &Menus,
    story: &street_sim::story::Story,
) -> Option<(u16, transition::Kind)> {
    let page = if let Some(page) = menus.return_page {
        page
    } else {
        menus.stack.get(1).unwrap_or(&menus.stack[0]).0
    };
    let class = screen_of(page, menus, story)?;
    let kind = if matches!(class, 0x60 | 0x3b | 0x3c) {
        transition::Kind::Forward
    } else {
        transition::Kind::Back
    };
    Some((class, kind))
}

/// Starts a screen's transition when the page on show changes screen (a
/// page deeper in the stack goes forward, one shallower back), and when
/// the menus hand over to a game's load (forward, the load's movie, the
/// game's state once it has played).
fn watch(
    mut menus: ResMut<Menus>,
    save: Res<story::Save>,
    mut t: ResMut<transition::Transition>,
    mut next: ResMut<NextState<Screen>>,
    time: Res<Time<Real>>,
) {
    let now = time.elapsed_secs_f64();
    let (page, _) = menus.under_dialog().unwrap_or(menus.top());
    let depth = menus.stack.len();
    let class = screen_of(page, &menus, &save.story);
    let legend = menus.players.mode == crate::choose::Mode::Legend;
    let last = t.last;
    // Main opens on Play It whichever way it is reached (its build
    // focuses its list's first row; measured after Options, High Scores
    // and Play It).
    if class == Some(0x03) && last.map(|(c, _)| c) != Some(0x03) && page == Page::Main {
        *menus.cursor() = 0;
    }
    // So does My Team on Create A Baller (seen after Create Team's save).
    if class == Some(0x06) && last.map(|(c, _)| c) != Some(0x06) && page == Page::MyTeam {
        *menus.cursor() = 0;
    }
    if let Some(class) = class {
        t.last = Some((class, depth));
    }
    if !t.idle() {
        return;
    }
    // A title standing: later titles come with the intro and the zoom (the
    // front end's `+0x6c`), also where the boot's movies were not shown.
    if class == Some(0x63) {
        t.titled = true;
    }
    if let (NextState::Pending(Screen::Loading), Some(from)) = (&*next, class) {
        next.reset();
        t.begin(
            transition::Change::new(
                from,
                0x2c,
                transition::Kind::Forward,
                legend,
                Some(Screen::Loading),
            ),
            now,
        );
        return;
    }
    // A DVD page's movies: the page goes forward into itself
    // (`FUN_002d7cb0` on its own class), built anew on its first row.
    if let (Some(files), Some(class)) = (menus.dvd.take(), class) {
        *menus.cursor() = 0;
        t.begin(
            transition::Change {
                dvd: Some(files),
                ..transition::Change::new(class, class, transition::Kind::Forward, legend, None)
            },
            now,
        );
        return;
    }
    if let (Some((from, before)), Some(to)) = (last, class)
        && from != to
    {
        let kind = if depth < before {
            transition::Kind::Back
        } else {
            transition::Kind::Forward
        };
        t.begin(
            transition::Change {
                hold_ms: std::mem::take(&mut menus.hold_ms),
                whoosh: menus
                    .whoosh
                    .take()
                    .unwrap_or_else(|| transition::hand_whoosh(from, to, kind)),
                ..transition::Change::new(from, to, kind, legend, None)
            },
            now,
        );
    }
}

/// Street School's one game: its set-up ([`crate::school::prepare`]),
/// opening on the first lesson's row; each lesson counts its attempt when
/// the lesson menu launches it.
fn start_school(
    disc: &Disc,
    setup: &mut MatchSetup,
    running: &mut story::Running,
    message: &mut Message,
) {
    setup.demo = setup.watch;
    match crate::school::prepare(disc, setup) {
        Ok(()) => running.0 = Some(story::Run::School(0)),
        Err(e) => message.0 = e,
    }
}

/// The premade picker's baller `k` (create-a-player record
/// `BALLER_RECORDS + k`) in the 3D preview (`FUN_00278098`:
/// `FUN_001e6518`, `FUN_002dbdd0`).
fn show_premade(editing: &mut ballers::Editing, disc: &Disc, k: usize) {
    let record = usize::from(street_sim::story::BALLER_RECORDS) + k;
    // The premade as its record has it (`FUN_001e6518`), its own outfit
    // too.
    editing.baller = disc.players.get(record).map(|p| {
        let mut b = ballers::new(k, p);
        b.clothing = (p.clothing & 0x3fff) as u8;
        b.clothing_variant = ((p.clothing as u16) >> 14).min(2) as u8;
        b
    });
}

/// The career's baller in the editor (Edit Baller from the map or from
/// Choose Baller's dialog: the edit screen, `FUN_00273dc8`).
fn edit_star(
    editing: &mut ballers::Editing,
    progress: &street_sim::story::Story,
    disc: &Disc,
    menus: &mut Menus,
    message: &mut Message,
) {
    let star = progress.crew.first().map(|&r| usize::from(r));
    let first = usize::from(street_sim::story::BALLER_RECORDS);
    match star.and_then(|r| r.checked_sub(first)) {
        Some(slot)
            if slot < street_sim::story::BALLER_SLOTS
                || progress.ballers.iter().any(|b| usize::from(b.slot) == slot) =>
        {
            open_baller(slot, editing, progress, disc, menus);
        }
        _ => message.0 = "Pick your crew first".into(),
    }
}

/// Opens baller `slot` in the editor: a saved one on its edit page (the
/// edit screen's pricing, against what it wears), else a new one from its
/// record in the wizard.
fn open_baller(
    slot: usize,
    editing: &mut ballers::Editing,
    progress: &street_sim::story::Story,
    disc: &Disc,
    menus: &mut Menus,
) {
    let record = usize::from(street_sim::story::BALLER_RECORDS) + slot;
    editing.move_spent = 0;
    editing.start = None;
    editing.skill_spend = 0;
    let saved = progress
        .ballers
        .iter()
        .find(|b| usize::from(b.slot) == slot)
        .cloned();
    // A career's premade star has no baller saved yet: the editor opens on
    // its record as the record has it (its outfit owned and paid).
    let Some(baller) = saved.or_else(|| {
        disc.players.get(record).map(|p| {
            let mut b = ballers::new(slot, p);
            b.first_name = p.first_name.clone();
            b.clothing = (p.clothing & 0x3fff) as u8;
            b.clothing_variant = ((p.clothing as u16) >> 14).min(2) as u8;
            b.wardrobe = b.worn_wardrobe();
            b
        })
    }) else {
        return;
    };
    editing.paid = ballers::price(&baller);
    editing.worn = Some(baller.outfit_row());
    editing.baller = Some(baller);
    editing.skill = 0;
    menus.edit_panel = 0;
    menus.wizard_top = 0;
    menus.push(Page::EditBaller(slot), 0);
}

/// How the 3D baller is framed on the page shown (`FUN_002745d8`'s
/// states): the wizard's Look panels on the head, the premade picker chest
/// up, the rest whole.
pub(crate) fn preview_framing(menus: &Menus) -> crate::cab_view::Framing {
    let (page, _) = menus.under_dialog().unwrap_or(menus.top());
    match page {
        Page::Wizard(_, at) => crate::cab_view::Step::at(at).framing(),
        Page::EditBaller(_) => crate::cab_view::Edit::at(menus.edit_panel).framing(),
        Page::NewCareer(_) => crate::cab_view::Framing::Picker,
        _ => crate::cab_view::Framing::Body,
    }
}

/// Create Baller's wizard on a new baller in `slot` (`FUN_00273a08(screen,
/// 3, gender)`: class 0x4a for a man, 0x4b a woman), on its first panel.
fn start_wizard(
    slot: usize,
    female: bool,
    editing: &mut ballers::Editing,
    disc: &Disc,
    menus: &mut Menus,
) {
    let record = usize::from(street_sim::story::BALLER_RECORDS);
    let Some(p) = disc
        .players
        .get(record + slot.min(street_sim::story::BALLER_SLOTS - 1))
    else {
        return;
    };
    editing.move_spent = 0;
    editing.skill_spend = 0;
    editing.paid = 0;
    editing.worn = None;
    editing.skill = 0;
    editing.start = Some(ballers::START_POINTS);
    editing.baller = Some(ballers::fresh(slot, p, female));
    menus.wizard_top = 0;
    menus.wizard_icon_top = 0;
    menus.push(Page::Wizard(slot, 0), 0);
}

/// A star slot for a new career's own baller: the first career-only one
/// (6..9, past My Team's six, as the original's careers each keep their
/// player), else a free Create A Baller slot.
fn career_star_slot(story: &street_sim::story::Story) -> Option<usize> {
    let used = |s: usize| {
        story.ballers.iter().any(|b| usize::from(b.slot) == s)
            || (0..street_sim::story::CAREER_SLOTS).any(|c| {
                story.career_at(c).is_some_and(|career| {
                    career.crew.first() == Some(&(street_sim::story::BALLER_RECORDS + s as u16))
                })
            })
    };
    (street_sim::story::BALLER_SLOTS..street_sim::story::STAR_SLOTS)
        .chain(0..street_sim::story::BALLER_SLOTS)
        .find(|&s| !used(s))
}

/// Edit Baller's keys (the editor's panels over `FUN_0028c798`): L1 and
/// R1 go round the panels, up and down and left and right move their
/// lists (each move sets the baller; its price is paid when it is saved),
/// Start (Enter) leaves saving the baller ("Exit", layout 79's legend
/// 0x300d), Circle opens Edit Moves (the panels' legend, 0x3102), Triangle
/// leaves without saving.
fn editor_keys(
    input: &MenuInput,
    menus: &mut Menus,
    editing: &mut ballers::Editing,
    save: &mut story::Save,
    message: &mut Message,
    names: &ballers::Names,
    look: Option<&Look>,
) {
    use crate::cab_view::Edit;
    let Some(female) = editing.baller.as_ref().map(|b| b.female) else {
        return;
    };
    let edit = Edit::at(menus.edit_panel);
    let turn = input.shoulder();
    if turn != 0 {
        menus.edit_panel = edit.turn(turn, female).index();
        menus.wizard_top = 0;
        message.0.clear();
        return;
    }
    let dy = i32::from(input.down()) - i32::from(input.up());
    let dx = i32::from(input.right()) - i32::from(input.left());
    if dy != 0 || dx != 0 {
        ballers::edit_move(editing, edit, dy, dx, names);
        if let (Some(b), Some(look)) = (editing.baller.as_ref(), look) {
            let (items, at) = match edit.step() {
                Some(step) => crate::cab_view::list(look, b, step, &editing.outfit_extras),
                None => crate::cab_view::shoe_list(look, b),
            };
            let shown = look
                .layouts
                .iter()
                .find(|l| l.class == edit.layout())
                .map_or(5, |l| {
                    l.elements
                        .iter()
                        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&20))
                        .count()
                });
            menus.wizard_top = crate::cab_view::scrolled(menus.wizard_top, at, items.len(), shown);
        }
        return;
    }
    if input.circle_key() || input.pad_circle() {
        open_moves(menus, false);
        return;
    }
    let slot = match menus.top().0 {
        Page::EditBaller(slot) => slot,
        _ => 0,
    };
    // Start ("Exit") asks first (`FUN_0028c0b8`).
    if input.any(&[KeyCode::Enter], &[GamepadButton::Start]) {
        message.0.clear();
        ask(menus, slot, crate::cab_view::CabAsk::Exit);
        return;
    }
    // Cross on an outfit or shoes not owned asks to buy them
    // (`FUN_0028e710`, `FUN_00295e40`); the panels' Triangle does nothing
    // (the editor base's `FUN_0028e140`).
    if input.any(&[KeyCode::Space, KeyCode::KeyJ], &[GamepadButton::South])
        && matches!(edit, Edit::Outfit | Edit::Shoes)
        && let (Some(b), Some(look)) = (editing.baller.as_ref(), look)
    {
        let cost = ballers::edit_price(editing, edit);
        if cost > 0 {
            let name = match edit {
                Edit::Outfit => {
                    let (items, at) = crate::cab_view::list(
                        look,
                        b,
                        crate::cab_view::Step::Outfit,
                        &editing.outfit_extras,
                    );
                    items.get(at).cloned().unwrap_or_default()
                }
                _ => {
                    let (items, at) = crate::cab_view::shoe_list(look, b);
                    items.get(at).cloned().unwrap_or_default()
                }
            };
            let question = if ballers::points_left(editing, &save.story) < 0 {
                crate::cab_view::CabAsk::Poor { name }
            } else {
                crate::cab_view::CabAsk::Buy {
                    cost: cost as u32,
                    name,
                }
            };
            ask(menus, slot, question);
        }
    }
}

/// The credits' keys: their screen's (`FUN_00283d08`) Cross to NuFX's
/// roll, Circle (the keyboard's O) to the developer's, Triangle back to
/// Options; a roll's (`FUN_00282350`) Cross or Triangle back to Options
/// once its picture is in. Start and the rest do nothing.
fn credits_keys(input: &MenuInput, menus: &mut Menus) {
    let triangle = input.any(
        &[KeyCode::Escape, KeyCode::Backspace],
        &[GamepadButton::North],
    );
    match menus.top().0 {
        Page::Credits => {
            if input.cross() {
                menus.credits = None;
                menus.push(Page::CreditsRoll(crate::credits::Team::NuFx), 0);
            } else if input.pad_circle() || input.circle_key() {
                menus.credits = None;
                menus.push(Page::CreditsRoll(crate::credits::Team::CoreTeam), 0);
            } else if triangle {
                menus.stack.pop();
            }
        }
        Page::CreditsRoll(_) => {
            let started = menus.credits.as_ref().is_some_and(|r| r.started());
            if started && (input.cross() || triangle) {
                menus.leave_credits();
            }
        }
        _ => {}
    }
}

/// The Options screen's keys (`FUN_002a4d18`): up and down walk list 55
/// (it does not wrap), left and right step the focused option, Cross opens
/// Controller Layout (class 0x37), Game Data (0x61), Trax (0x30) or, for
/// Adjust Screen, the rewrite's own settings, and on Exit keeps the
/// settings and leaves; Start keeps and leaves as Exit; Triangle cancels,
/// asking first when an option changed ("Do you want to cancel your
/// changes?", `options.txt` 46, No lit; Yes puts them back); Circle opens
/// the credits (class 9); Select asks "Exit to Main Menu?" (`FUN_0024bfa0(..,
/// 0x85)`, before these keys). The rewrite's keyboard: Escape for Triangle,
/// O for Circle.
/// Rewards (`FUN_002c1370`): the arrows step the grid's cards or the
/// trophies, L1 and R1 the tabs; Cross on a locked card asks to unlock it
/// (or says the points are short), Yes buying it as the shop always has
/// (a development card going on to its recipient); Start leaves (the
/// keyboard's Escape too); Square is Help.
fn rewards_keys(
    input: &MenuInput,
    menus: &mut Menus,
    save: &mut story::Save,
    disc: &Disc,
    look: Option<&Look>,
    message: &mut Message,
) {
    use crate::rewards_view as rv;
    let (page, cursor) = menus.top();
    if page == Page::RewardAsk {
        let rows = match menus.reward_ask {
            Some(rv::Ask::Unlock { .. }) => 2,
            _ => 1,
        };
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            *menus.cursor() = (cursor as i32 + dy).clamp(0, rows - 1) as usize;
            return;
        }
        if input.back() {
            menus.reward_ask = None;
            menus.stack.pop();
            return;
        }
        if !input.confirm() {
            return;
        }
        let ask = menus.reward_ask.take();
        menus.stack.pop();
        if let (Some(rv::Ask::Unlock { card, .. }), 0) = (ask, cursor) {
            let s = menus.rewards;
            let Some(found) = disc.cards.iter().find(|c| c.id == card) else {
                return;
            };
            if found.category == crate::rewards::DEVELOPMENT {
                if crate::rewards::development_recipients(&save.story).is_empty() {
                    message.0 = "Create a baller or start a career first.".into();
                } else {
                    menus.push(Page::DevelopmentRecipient(card as usize), 0);
                }
                return;
            }
            match story::reward_buy(&mut save.story, disc, s.tab, s.index() + 1) {
                Ok(_) => {
                    save.store();
                    menus.rewards_bought = true;
                }
                Err(note) => message.0 = note,
            }
        }
        return;
    }
    let mut s = menus.rewards;
    // The close-up: left and right step the cards one may look at, round;
    // Circle turns a legend's card to its back and again (O on the
    // keyboard, or a pad's Circle); Triangle goes back to
    // the grid, and Cross too, except on development points, whose Cross
    // picks the baller they go to.
    if let Some(v) = s.view {
        let all = rv::viewable(&save.story, disc);
        let dx = i32::from(input.right()) - i32::from(input.left());
        if dx != 0 && all.len() > 1 {
            s.view = Some((v as i32 + dx).rem_euclid(all.len() as i32) as usize);
            s.turn = false;
        } else if input.circle_key() || input.pad_circle() {
            if all.get(v).and_then(|c| rv::back_picture(c)).is_some() {
                s.turn = true;
            }
        } else if input.back() {
            s.view = None;
        } else if input.confirm() {
            match all.get(v) {
                Some(card) if card.category == crate::rewards::DEVELOPMENT => {
                    if crate::rewards::development_recipients(&save.story).is_empty() {
                        message.0 = "Create a baller or start a career first.".into();
                    } else {
                        menus.push(Page::DevelopmentRecipient(card.id as usize), 0);
                    }
                }
                _ => s.view = None,
            }
        }
        menus.rewards = s;
        return;
    }
    let leave = input.any(
        &[KeyCode::Escape, KeyCode::Backspace],
        &[GamepadButton::Start],
    );
    let act = input.any(
        &[KeyCode::Enter, KeyCode::Space, KeyCode::KeyJ],
        &[GamepadButton::South],
    );
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    if input.shoulder() != 0 {
        rv::step_tab(&mut s, input.shoulder());
    } else if s.tab == rv::TROPHIES {
        if dy != 0 {
            rv::step_trophy(&mut s, dy);
        }
    } else if dx != 0 || dy != 0 {
        let n = rv::cards(disc, s.tab).len();
        rv::step_grid(&mut s, n, dy, dx);
    } else if act && s.tab != rv::TROPHIES {
        let all = rv::cards(disc, s.tab);
        if let Some(card) = all.get(s.index()) {
            if crate::rewards::card_owned(&save.story, card) {
                s.view = rv::viewable(&save.story, disc)
                    .iter()
                    .position(|c| c.id == card.id);
            } else if let Some(look) = look
                && let Some(ask) = rv::ask(look, &save.story, disc, &s)
            {
                menus.reward_ask = Some(ask);
                menus.push(Page::RewardAsk, 0);
            }
        }
    } else if leave {
        menus.stack.pop();
        // Exit after a purchase saves (`FUN_002c1370`'s 0x22 with
        // `+0x2b0`), else straight to the Main Menu.
        if std::mem::take(&mut menus.rewards_bought) {
            menus.save_to_card();
        }
    }
    menus.rewards = s;
}

/// Rewards' close-up: the card's big picture (`XAFERWD`, `FUN_002c2fb8`'s
/// `FUN_002e4560(.., 1, picture, 4)`) and a legend's back (`XARWDBK`, its
/// `FUN_002e4560(.., 3, back, 3)`), decoded when the close-up asks for
/// them; the archives stay decoded while the screen is up. The Info turn
/// runs here, one step per front-end update (60 a second).
#[allow(clippy::too_many_arguments)]
fn reward_card_picture(
    mut menus: ResMut<Menus>,
    mut disc: ResMut<Disc>,
    save: Res<story::Save>,
    time: Res<Time<Real>>,
    mut images: ResMut<Assets<Image>>,
    mut big: ResMut<crate::rewards_view::BigCard>,
    mut turn: ResMut<crate::rewards_view::Turn>,
    mut sheets: Local<[Option<Vec<street_assets::ssh::Sheet>>; 2]>,
) {
    use crate::rewards_view as rv;
    let on = matches!(
        menus.top().0,
        Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_)
    );
    if !on {
        *sheets = [None, None];
    }
    let card = menus
        .rewards
        .view
        .filter(|_| on)
        .and_then(|v| rv::viewable(&save.story, &disc).get(v).copied().cloned());
    let want = card.as_ref().map(rv::big_picture);
    let decode =
        |sheets: &Option<Vec<street_assets::ssh::Sheet>>, k: usize, images: &mut Assets<Image>| {
            sheets
                .as_ref()
                .and_then(|all| all.get(k))
                .and_then(|sheet| crate::look::sprite_crop(sheet, 0, 0))
                .map(|t| {
                    let size = Vec2::new(t.width as f32, t.height as f32);
                    (images.add(street_render::convert::image(&t)), size)
                })
        };
    if big.0.as_ref().map(|b| b.0) != want {
        *turn = rv::Turn::default();
        menus.rewards.turn = false;
        big.0 = None;
        if let Some(k) = want {
            for (slot, name) in [(0, "/FE/XAFERWD.BIN"), (1, "/FE/XARWDBK.BIN")] {
                if sheets[slot].is_none() {
                    sheets[slot] = street_assets::fe::read(&mut disc.iso, name).ok();
                }
            }
            big.0 = decode(&sheets[0], k, &mut images).map(|(h, size)| (k, h, size));
            turn.back = card
                .as_ref()
                .and_then(rv::back_picture)
                .and_then(|b| decode(&sheets[1], b, &mut images).map(|(h, _)| (b, h)));
        }
    }
    // The turn: Circle's ask starts it; it swaps the faces half way.
    if menus.rewards.turn && turn.back.is_some() && turn.began.is_none() {
        turn.began = Some(time.elapsed_secs());
    }
    let shown = turn.back_shown;
    (turn.face_back, turn.squeeze) = (shown, 0.0);
    if let Some(at) = turn.began {
        let n = ((time.elapsed_secs() - at) * 60.0) as u32;
        match rv::turn_at(n) {
            Some((t, swapped)) => (turn.face_back, turn.squeeze) = (shown ^ swapped, 2.0 * t),
            None => {
                turn.back_shown = !shown;
                turn.face_back = !shown;
                turn.began = None;
                menus.rewards.turn = false;
            }
        }
    }
}

/// Trax (`FUN_00265978` and the radio's own keys): up and down along
/// the rows, the list scrolling one row at a time; left and right change
/// the row's choice; Cross previews a Playlist song (the rewrite cannot
/// yet play an Interactive one alone) and leaves on Exit; Start leaves
/// keeping the changes; Triangle leaves, asking first when something
/// changed ("Do you want to cancel your changes?": Yes puts them back).
fn trax_keys(
    input: &MenuInput,
    menus: &mut Menus,
    opts: &mut options::Options,
    commands: &mut Commands,
) {
    let (page, cursor) = menus.top();
    let before = *menus.trax_before.get_or_insert(opts.0);
    let leave = |menus: &mut Menus| {
        menus.trax_before = None;
        // The list keeps its place under the card's dialogs.
        if menus.card_request.is_none() {
            menus.trax_top = 0;
        }
        menus.stack.pop();
    };
    // Leaving with the songs changed applies them and saves
    // (`FUN_00265978` case 0x18: `FUN_0021a108`, `FUN_00219f58`).
    let keep = |menus: &mut Menus, opts: &options::Options| {
        if opts.0 != before {
            menus.save_to_card();
        }
    };
    if page == Page::TraxAsk {
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            *menus.cursor() = (cursor as i32 + dy).clamp(0, 1) as usize;
        } else if input.back() {
            menus.stack.pop();
        } else if input.confirm() {
            menus.stack.pop();
            if cursor == 0 {
                opts.0 = before;
                leave(menus);
            }
        }
        return;
    }
    let rows = crate::trax_view::rows(&opts.0);
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dy != 0 {
        let to = (cursor as i32 + dy).clamp(0, rows.len() as i32 - 1) as usize;
        *menus.cursor() = to;
        menus.trax_top =
            crate::cab_view::scrolled(menus.trax_top, to, rows.len(), crate::trax_view::SHOWN);
        return;
    }
    let dx = i32::from(input.right()) - i32::from(input.left());
    if dx != 0 {
        if let Some(&row) = rows.get(cursor) {
            crate::trax_view::step(&mut opts.0, row);
            // The other experience's songs replace these.
            let n = crate::trax_view::rows(&opts.0).len();
            menus.trax_top = crate::cab_view::scrolled(
                menus
                    .trax_top
                    .min(n.saturating_sub(crate::trax_view::SHOWN)),
                cursor.min(n - 1),
                n,
                crate::trax_view::SHOWN,
            );
        }
        return;
    }
    if input.any(&[], &[GamepadButton::Start]) {
        keep(menus, opts);
        leave(menus);
        return;
    }
    if input.back() {
        if opts.0 != before {
            menus.push(Page::TraxAsk, 1);
        } else {
            leave(menus);
        }
        return;
    }
    if input.confirm() {
        match rows.get(cursor) {
            Some(crate::trax_view::Row::Exit) => {
                keep(menus, opts);
                leave(menus);
            }
            Some(&crate::trax_view::Row::Song { bit, .. })
                if bit >= street_sim::records::FRONT_END_BIT =>
            {
                let k = (bit - street_sim::records::FRONT_END_BIT) as usize;
                if let Some(&path) = street_audio::music::FRONT_END_SONGS.get(k) {
                    commands.insert_resource(crate::audio::Preview(Some(path)));
                }
            }
            _ => {}
        }
    }
}

fn options_keys(
    input: &MenuInput,
    menus: &mut Menus,
    save: &mut story::Save,
    (opts, sound, pads, setup): (
        &mut options::Options,
        &mut AudioSettings,
        &crate::pads::PadSettings,
        &mut MatchSetup,
    ),
) {
    use options::row;
    let (page, cursor) = menus.top();
    let now = options::gather(&opts.0, sound, pads, setup.rules.difficulty);
    let before = *menus.options_before.get_or_insert(now);
    // What leaving keeps: the settings into the game data, and the card's
    // save when they changed (the rewrite's PC settings count, being its
    // Adjust Screen's).
    let keep = |menus: &mut Menus,
                save: &mut story::Save,
                opts: &options::Options,
                sound: &AudioSettings,
                setup: &MatchSetup| {
        let now = options::gather(&opts.0, sound, pads, setup.rules.difficulty);
        save.story.settings = Some(now);
        save.store();
        if options::changed(&before, &now) || options::pc_changed(&before, &now) {
            menus.save_to_card();
        }
    };
    let leave = |menus: &mut Menus,
                 save: &mut story::Save,
                 opts: &options::Options,
                 sound: &AudioSettings,
                 setup: &MatchSetup| {
        keep(menus, save, opts, sound, setup);
        menus.options_before = None;
        // The list keeps its place under the card's dialogs.
        if menus.card_request.is_none() {
            menus.options_top = 0;
        }
        menus.stack.pop();
    };
    if page == Page::OptionsAsk {
        let dy = i32::from(input.down()) - i32::from(input.up());
        if dy != 0 {
            *menus.cursor() = (cursor as i32 + dy).clamp(0, 1) as usize;
            return;
        }
        if input.confirm() || input.back() {
            menus.stack.pop();
            if input.confirm() && cursor == 0 {
                // Yes: the settings as they were.
                put_back_options(before, opts, sound, &mut setup.rules.difficulty);
                leave(menus, save, opts, sound, setup);
            }
        }
        return;
    }
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dy != 0 {
        let to = (cursor as i32 + dy).clamp(0, options::ROWS as i32 - 1) as usize;
        *menus.cursor() = to;
        menus.options_top = crate::cab_view::scrolled(
            menus.options_top,
            to,
            options::ROWS,
            crate::options_view::SHOWN,
        );
        return;
    }
    let dx = i32::from(input.right()) - i32::from(input.left());
    if dx != 0 {
        options::step(cursor, dx, &mut opts.0, sound, &mut setup.rules.difficulty);
        return;
    }
    if input.circle_key() || input.pad_circle() {
        menus.push(Page::Credits, 0);
        return;
    }
    if input.any(&[], &[GamepadButton::Start]) {
        leave(menus, save, opts, sound, setup);
        return;
    }
    if input.back() {
        if options::changed(&before, &now) {
            menus.push(Page::OptionsAsk, 1);
        } else {
            menus.options_before = None;
            menus.options_top = 0;
            menus.stack.pop();
        }
        return;
    }
    if input.confirm() {
        match cursor {
            row::LAYOUT => menus.push(Page::Layouts, 0),
            row::GAME_DATA => {
                keep(menus, save, opts, sound, setup);
                menus.options_before = None;
                menus.push(Page::GameData, 0);
            }
            row::TRAX => {
                keep(menus, save, opts, sound, setup);
                menus.options_before = None;
                menus.trax_before = Some(opts.0);
                menus.trax_top = 0;
                menus.push(Page::Trax, 0);
            }
            row::SCREEN => menus.push(Page::PcSettings, 0),
            row::EXIT => leave(menus, save, opts, sound, setup),
            _ => {}
        }
    }
}

/// Opens a question over the wizard or the editor, on its opening row.
fn ask(menus: &mut Menus, slot: usize, question: crate::cab_view::CabAsk) {
    let row = question.opening_row();
    menus.cab_ask = Some(question);
    menus.push(Page::CabAsk(slot), row);
}

/// The questions' keys: up and down between Yes and No, Cross answers,
/// Triangle is No. Yes leaves the wizard (or backs up to the premade
/// picker), buys the editor's item, or saves the baller and leaves Edit
/// Baller (whose changes the original keeps as they are made).
fn cab_ask_keys(
    input: &MenuInput,
    menus: &mut Menus,
    editing: &mut ballers::Editing,
    save: &mut story::Save,
    disc: &Disc,
    message: &mut Message,
) {
    use crate::cab_view::CabAsk;
    let (_, cursor) = menus.top();
    let Some(question) = menus.cab_ask.clone() else {
        menus.stack.pop();
        return;
    };
    let rows = if question.opening_row() == 1 { 2 } else { 1 };
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dy != 0 {
        *menus.cursor() = (cursor as i32 + dy).clamp(0, rows - 1) as usize;
        return;
    }
    let yes = input.confirm() && rows == 2 && cursor == 0;
    if !(input.confirm() || input.back()) {
        return;
    }
    menus.stack.pop();
    menus.cab_ask = None;
    if !yes {
        return;
    }
    match question {
        CabAsk::Leave { back_up } => {
            // Back to the premade picker (on its premade), or out of
            // Create Baller.
            editing.start = None;
            menus.wizard_career = None;
            menus.stack.pop();
            match (back_up, menus.top()) {
                (true, (Page::NewCareer(_), at)) => show_premade(editing, disc, at),
                _ => editing.baller = None,
            }
        }
        CabAsk::Buy { .. } => {
            let edit = crate::cab_view::Edit::at(menus.edit_panel);
            ballers::edit_buy(editing, edit);
        }
        CabAsk::Exit => {
            let before = save.story.ballers.clone();
            match ballers::commit(editing, &mut save.story) {
                Ok(()) => {
                    save.store();
                    message.0.clear();
                    menus.stack.pop();
                    // A changed baller saves (`FUN_0028c3f0`,
                    // `DAT_003f93d0`).
                    if save.story.ballers != before {
                        menus.save_to_card();
                    }
                }
                Err(e) => message.0 = e,
            }
        }
        CabAsk::WizardPoor | CabAsk::Poor { .. } => {}
    }
}

/// Opens Edit Moves on its first category (`FUN_002914b8`): over Edit
/// Baller (its Circle), or `alone` from a baller's dialog
/// (`FUN_00290b40`'s screen, which saves and leaves to the list on Exit).
fn open_moves(menus: &mut Menus, alone: bool) {
    menus.moves = crate::edit_moves::Panel {
        alone,
        ..Default::default()
    };
    menus.push(Page::MoveSlots(0), 0);
}

/// Edit Moves' keys (`FUN_00292ce0` and the editor's base `FUN_0028ce30`):
/// step 1's up and down walk the slots, L1 and R1 the categories (the
/// header wraps), Cross picks the slot, Start leaves ("Exit"); step 2's up
/// and down walk the moves, Cross takes one (or asks), Triangle goes back;
/// Circle previews the focused move in either step. The rewrite's keyboard:
/// Enter, Space or J for Cross, Escape for Start in step 1 and Triangle in
/// step 2, O for Circle.
fn moves_keys(
    input: &MenuInput,
    menus: &mut Menus,
    editing: &mut ballers::Editing,
    save: &mut story::Save,
    disc: &Disc,
    message: &mut Message,
) {
    use crate::edit_moves::{self as em, Category};
    let (page, cursor) = menus.top();
    // The Moves (`FUN_00295460`: L1 and R1 the categories; the base's
    // Triangle leaves; Cross previews, `FUN_00294e28`).
    if let Page::TheMoves(index) = page {
        let cat = Category::at(index);
        let list = em::browse_list(cat);
        let dy = i32::from(input.down()) - i32::from(input.up());
        let turn = input.shoulder();
        if turn != 0 {
            let n = em::CATEGORIES.len() as i32;
            let to = (index as i32 + turn).rem_euclid(n) as usize;
            if let Some(top) = menus.stack.last_mut() {
                *top = (Page::TheMoves(to), 0);
            }
            menus.moves.pick_top = 0;
            menus.moves.preview = None;
        } else if dy != 0 {
            let to = (cursor as i32 + dy).clamp(0, list.len().max(1) as i32 - 1) as usize;
            if to != cursor {
                *menus.cursor() = to;
                menus.moves.pick_top = crate::cab_view::scrolled(
                    menus.moves.pick_top,
                    to,
                    list.len(),
                    em::PICKS_SHOWN,
                );
                menus.moves.preview = None;
            }
        } else if input.back() {
            menus.moves = Default::default();
            menus.stack.pop();
        } else if input.confirm() {
            let row = list.get(cursor).copied();
            menus.moves.preview = row.filter(|&r| !em::browse_locked(disc, &save.story, r));
        }
        return;
    }
    let Some(b) = editing.baller.clone() else {
        menus.stack.pop();
        return;
    };
    let pad_start = input.any(&[], &[GamepadButton::Start]);
    let circle = input.circle_key() || input.pad_circle();
    let dy = i32::from(input.down()) - i32::from(input.up());
    match page {
        Page::MoveSlots(index) => {
            let cat = Category::at(index);
            let count = cat.slots().len();
            let turn = input.shoulder();
            if turn != 0 {
                let n = em::CATEGORIES.len() as i32;
                let to = (index as i32 + turn).rem_euclid(n) as usize;
                if let Some(top) = menus.stack.last_mut() {
                    *top = (Page::MoveSlots(to), 0);
                }
                menus.moves.slots_top = 0;
                menus.moves.preview = None;
                return;
            }
            if dy != 0 {
                let to = (cursor as i32 + dy).clamp(0, count as i32 - 1) as usize;
                if to != cursor {
                    *menus.cursor() = to;
                    menus.moves.slots_top = crate::cab_view::scrolled(
                        menus.moves.slots_top,
                        to,
                        count,
                        em::SLOTS_SHOWN,
                    );
                    menus.moves.preview = None;
                }
                return;
            }
            if circle {
                menus.moves.preview = em::slot_moves(disc, &b, cat)
                    .get(cursor)
                    .copied()
                    .filter(|&r| r != street_sim::story::EMPTY_MOVE_SLOT);
                return;
            }
            if pad_start || input.any(&[KeyCode::Escape], &[]) {
                menus.moves.preview = None;
                menus.stack.pop();
                // From a baller's dialog the moves are the baller's at once.
                if menus.moves.alone && matches!(menus.top().0, Page::EditBaller(_)) {
                    let before = save.story.ballers.clone();
                    match ballers::commit(editing, &mut save.story) {
                        Ok(()) => {
                            save.store();
                            menus.stack.pop();
                            // Changed moves save (`FUN_0028c3f0`).
                            if save.story.ballers != before {
                                menus.save_to_card();
                            }
                        }
                        Err(e) => message.0 = e,
                    }
                }
                return;
            }
            if input.confirm() {
                menus.moves.pick_top = 0;
                menus.moves.preview = None;
                menus.push(Page::MovePick(index, cursor), 0);
            }
        }
        Page::MovePick(index, slot) => {
            let cat = Category::at(index);
            let list = em::pick_list(disc, &save.story, &b, cat);
            if dy != 0 {
                let to = (cursor as i32 + dy).clamp(0, list.len().max(1) as i32 - 1) as usize;
                if to != cursor {
                    *menus.cursor() = to;
                    menus.moves.pick_top = crate::cab_view::scrolled(
                        menus.moves.pick_top,
                        to,
                        list.len(),
                        em::PICKS_SHOWN,
                    );
                    menus.moves.preview = None;
                }
                return;
            }
            if circle {
                // Only a move to be had previews (`FUN_00292ce0`).
                menus.moves.preview = list
                    .get(cursor)
                    .copied()
                    .filter(|&r| em::standing(disc, &save.story, &b, r) != em::Standing::Locked);
                return;
            }
            if input.back() {
                menus.moves.preview = None;
                menus.stack.pop();
                return;
            }
            if input.confirm() && !pad_start {
                let Some(&row) = list.get(cursor) else {
                    return;
                };
                match em::select(disc, &save.story, editing, cat, slot, row) {
                    Ok(()) => {
                        menus.moves.preview = None;
                        menus.stack.pop();
                    }
                    Err(ask) => {
                        menus.moves.ask = Some(ask);
                        menus.push(Page::MoveAsk(index, slot), ask.opening_row());
                    }
                }
            }
        }
        Page::MoveAsk(index, slot) => {
            let Some(ask) = menus.moves.ask else {
                menus.stack.pop();
                return;
            };
            let rows = if matches!(ask, em::Ask::Buy { .. }) {
                2
            } else {
                1
            };
            if dy != 0 {
                *menus.cursor() = (cursor as i32 + dy).clamp(0, rows - 1) as usize;
                return;
            }
            let answered = input.confirm() || input.back();
            if !answered {
                return;
            }
            menus.stack.pop();
            menus.moves.ask = None;
            // Yes buys the move and puts it in the slot (`FUN_00292bb8`).
            if let (em::Ask::Buy { row, .. }, 0, false) = (ask, cursor, input.back())
                && em::buy(disc, &save.story, editing, Category::at(index), slot, row)
            {
                menus.moves.preview = None;
                menus.stack.pop();
            }
        }
        _ => {}
    }
}

/// Create Baller's wizard's keys (`FUN_00275148`, the panels' +0x164 and
/// +0x16c): up and down move list 0x14, left and right list 0x2c (each
/// move sets the baller); Cross and R1 go on (Build and Outfit first check
/// the points, `FUN_00276d40`, `FUN_00278930`), Triangle and L1 go back;
/// the name panel types on its keyboard.
#[allow(clippy::too_many_arguments)]
fn wizard_keys(
    input: &MenuInput,
    menus: &mut Menus,
    editing: &mut ballers::Editing,
    story: &street_sim::story::Story,
    typing: &mut Typing,
    message: &mut Message,
    names: &ballers::Names,
    look: Option<&Look>,
    (slot, step): (usize, crate::cab_view::Step),
) {
    use crate::cab_view::Step;
    let Some(female) = editing.baller.as_ref().map(|b| b.female) else {
        return;
    };
    let set = |menus: &mut Menus, to: Step| {
        if let Some(top) = menus.stack.last_mut() {
            *top = (Page::Wizard(slot, to.index()), 0);
        }
        menus.wizard_top = 0;
    };
    // The name panel's keyboards: the typing owns the keys.
    if matches!(step, Step::First | Step::Last) {
        if let Some(b) = editing.baller.as_mut()
            && step == Step::First
            && b.first_name == "NAME"
        {
            // The keyboard opens on the default name (`entername.txt` 45).
            b.first_name = "PLAYER".into();
        }
        typing.field = Some(if step == Step::First {
            Field::CabFirst
        } else {
            Field::CabLast
        });
        // The keyboard opens on DONE (`my_team_wizard_first_name.png`).
        typing.key = (5, 0);
        return;
    }
    let dy = i32::from(input.down()) - i32::from(input.up());
    let dx = i32::from(input.right()) - i32::from(input.left());
    if dy != 0 || dx != 0 {
        ballers::wizard_move(editing, step, dy, dx, names);
        // List 0x14 keeps its cursor in view.
        if let (Some(b), Some(look)) = (editing.baller.as_ref(), look) {
            let (items, at) = crate::cab_view::list(look, b, step, &editing.outfit_extras);
            let shown = look
                .layouts
                .iter()
                .find(|l| l.class == step.layout())
                .map_or(5, |l| {
                    l.elements
                        .iter()
                        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&20))
                        .count()
                });
            menus.wizard_top = crate::cab_view::scrolled(menus.wizard_top, at, items.len(), shown);
            if step == Step::Icon {
                menus.wizard_icon_top =
                    crate::cab_view::scrolled(menus.wizard_icon_top, usize::from(b.icon), 7, 3);
            }
        }
        return;
    }
    let turn = input.shoulder();
    if input.confirm() || turn > 0 {
        if matches!(step, Step::Build | Step::Outfit) && ballers::points_left(editing, story) < 0 {
            ask(menus, slot, crate::cab_view::CabAsk::WizardPoor);
            return;
        }
        message.0.clear();
        match step.next(female) {
            Some(next) => set(menus, next),
            // The icon's next: "CREATE %1S?", CANCEL lit.
            None if turn == 0 => menus.push(Page::WizardAsk(slot), 2),
            None => {}
        }
        return;
    }
    if input.back() || turn < 0 {
        message.0.clear();
        match step.back(female) {
            Some(prev) => set(menus, prev),
            // Build's Triangle asks before it leaves (`FUN_00276f08`):
            // back up to the premade picker when it opened the wizard,
            // else exit Create Baller.
            None if turn == 0 => {
                let back_up = menus
                    .stack
                    .iter()
                    .any(|(p, _)| matches!(p, Page::NewCareer(_)));
                ask(menus, slot, crate::cab_view::CabAsk::Leave { back_up });
            }
            None => {}
        }
    }
}

/// What the game's look needs to draw a page.
#[derive(SystemParam)]
struct LookParams<'w, 's> {
    look: Option<Res<'w, Look>>,
    /// The page on screen and when it opened, for its rows' pulse.
    opened: Local<'s, Option<(Page, f32)>>,
    time: Res<'w, Time<Real>>,
    screen: Option<Res<'w, crate::bracket_view::ScreenArt>>,
    labels: ResMut<'w, look::Labels>,
    images: ResMut<'w, Assets<Image>>,
    /// The in-game layouts' art, for the dialog widget (`FUN_0025c860`)
    /// the front end's dialogs share with the pause screen.
    gm: Option<Res<'w, crate::boards::Gm>>,
    panels: Local<'s, crate::boards::Panels>,
    /// The players' portraits (Choose Players' grids).
    portraits: Option<ResMut<'w, crate::portraits::Portraits>>,
    /// Create Baller's panels' sprites.
    cab: Option<Res<'w, crate::cab_view::CabArt>>,
    /// Rewards' close-up picture and its Info turn.
    big_card: Res<'w, crate::rewards_view::BigCard>,
    turn: Res<'w, crate::rewards_view::Turn>,
    /// The memory card's dialog over the page.
    card: Res<'w, crate::card::Card>,
    /// The screens' transitions: the arriving screen's pose, and the page
    /// kept for its leaving.
    transition: ResMut<'w, transition::Transition>,
    /// The idle demo's "Loading Demo..." on the title and Main.
    idle: Res<'w, crate::attract::Idle>,
    commands: Commands<'w, 's>,
    slots: Query<
        'w,
        's,
        (
            &'static look::Slot,
            &'static mut ImageNode,
            &'static mut Node,
            &'static mut Visibility,
        ),
        Without<MenuBg>,
    >,
}

#[allow(clippy::too_many_arguments)]
fn draw(
    menus: Res<Menus>,
    disc: Res<Disc>,
    setup: Res<MatchSetup>,
    save: Res<story::Save>,
    (sound, pads, opts, sounds): (
        Res<AudioSettings>,
        Res<crate::pads::PadSettings>,
        Res<options::Options>,
        Res<crate::audio::Sounds>,
    ),
    editing: Res<ballers::Editing>,
    team_editing: Res<teams::TeamEditing>,
    typing: Res<Typing>,
    art: Option<Res<art::Art>>,
    mut bg: Query<&mut ImageNode, With<MenuBg>>,
    message: Res<Message>,
    mut title_text: Query<&mut Text, (With<TitleText>, Without<BodyText>, Without<HintText>)>,
    mut body_text: Query<&mut Text, (With<BodyText>, Without<TitleText>, Without<HintText>)>,
    mut hint_text: Query<&mut Text, (With<HintText>, Without<TitleText>, Without<BodyText>)>,
    mut fe: LookParams,
    film: Res<crate::movie::FilmRoom>,
) {
    let (page, cursor) = menus.top();
    // A dialog page draws over the screen it opened on.
    let (base, base_cursor) = menus.under_dialog().unwrap_or((page, cursor));
    let mut list = items(
        page,
        &menus,
        &disc,
        &setup,
        &save,
        (&sound, &pads, &opts.0, sounds.songs()),
        &editing,
        &team_editing,
    );
    // The game's own backdrop behind the page: the one the game's screen
    // for it shows.
    if let (Some(art), Ok(mut image)) = (art.as_ref(), bg.single_mut()) {
        use art::backdrop as b;
        let n = match page {
            Page::Title => b::TITLE,
            Page::Main | Page::PlayIt | Page::Difficulty => b::MAIN,
            // A list too wide for the main backdrop's picture panel.
            Page::School => b::OPTIONS,
            Page::ChoosePlayers | Page::StoryTeam | Page::UpdateTeam(_) => b::CHOOSE,
            Page::ChooseCourt | Page::PlayGameAsk => crate::choosecourt::BACKDROP,
            Page::MyRules | Page::RulesCancelAsk => crate::myrules::BACKDROP,
            Page::Options | Page::OptionsAsk | Page::PcSettings => b::OPTIONS,
            Page::Trax | Page::TraxAsk => crate::trax_view::BACKDROP,
            Page::Layouts => b::CONTROLS,
            Page::League | Page::LeagueRegion(_) => b::REGION,
            Page::LeagueTeam => crate::chooseteam::LEAGUE_BACKDROP,
            Page::Story | Page::City(_) => crate::map_view::BACKDROP,
            Page::Bracket(ev) => crate::bracket_view::backdrop_of(crate::bracket_view::class_of(
                crate::bracket_view::shown(&save.story, ev).kind,
            )),
            Page::EventIntro(_) => crate::event_intro::BACKDROP,
            Page::NewCareer(_) | Page::PremadeAsk(_) => b::CREATE,
            Page::Careers | Page::CareerAsk(_) | Page::Ballers | Page::BallerAsk(_) => {
                crate::careers::BACKDROP
            }
            Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_) => {
                if menus.rewards.view.is_some() {
                    crate::rewards_view::VIEW_BACKDROP
                } else {
                    crate::rewards_view::BACKDROP
                }
            }
            Page::Completed(mode) => crate::completed_view::backdrop_of(mode),
            Page::JoinTeam => crate::join_view::BACKDROP,
            Page::EditBaller(_) | Page::Wizard(..) | Page::WizardAsk(_) | Page::CabAsk(_) => {
                b::CREATE
            }
            // `FUN_00290b40`: the editor's screen over backdrop 12.
            Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..) => {
                crate::edit_moves::BACKDROP
            }
            // The DVD pages' own backdrop (`FUN_002d7720(.., 0x12)`); The
            // Moves is the edit-player moves screen (class 101).
            Page::Inside | Page::WhatsNew | Page::Bobbito => b::MAIN,
            Page::TheMoves(_) => crate::edit_moves::BACKDROP,
            Page::MyTeam => b::MAIN,
            Page::Teams
            | Page::EditTeam(_)
            | Page::PickPlayer
            | Page::StoryCrew
            | Page::CreateTeam => b::TEAM,
            Page::Controls => b::CONTROLS,
            // `FUN_002d7720(.., 9, 0x1f)`; the rolls' (`.., 7, 8`).
            Page::Credits => b::CREDITS,
            Page::CreditsRoll(_) => crate::credits::ROLL_BACKDROP,
            Page::Online => b::MAIN,
            Page::MatchBrowser | Page::GameData | Page::Delete(_) | Page::DataAsk => b::OPTIONS,
            Page::HighScores => crate::score_view::BACKDROP,
            Page::Sides | Page::Users => crate::users::BACKDROP,
            Page::ChooseTeam(_) => crate::chooseteam::BACKDROP,
        };
        let handle = art.backdrop(n);
        if image.image != handle {
            image.image = handle;
        }
        // Without the game's fonts the plain text needs the art dimmed.
        image.color = if fe.look.is_some() {
            Color::WHITE
        } else {
            Color::srgba(0.55, 0.55, 0.62, 0.5)
        };
    }
    // The prompts name the buttons of whatever was touched last.
    let style = menus.style;
    let keys = match page {
        Page::Title => String::new(),
        // Their layouts' own legends (kind 34) stand for the prompts.
        Page::Sides
        | Page::Users
        | Page::ChooseTeam(_)
        | Page::LeagueTeam
        | Page::Bracket(_)
        | Page::Completed(_)
        | Page::JoinTeam
        | Page::NewCareer(_)
        | Page::PremadeAsk(_)
        | Page::Wizard(..)
        | Page::WizardAsk(_)
        | Page::EditBaller(_)
        | Page::CabAsk(_)
        | Page::MoveSlots(_)
        | Page::MovePick(..)
        | Page::MoveAsk(..)
        | Page::TheMoves(_)
        | Page::Options
        | Page::OptionsAsk
        | Page::ChooseCourt
        | Page::PlayGameAsk
        | Page::MyRules
        | Page::RulesCancelAsk
        | Page::Careers
        | Page::CareerAsk(_)
        | Page::BallerAsk(_)
        | Page::EventIntro(_)
        | Page::Story
        | Page::City(_)
        | Page::Trax
        | Page::TraxAsk
        | Page::Rewards
        | Page::RewardAsk
        | Page::DevelopmentRecipient(_)
        | Page::HighScores
        | Page::GameData
        | Page::Delete(_)
        | Page::DataAsk
        | Page::Credits
        | Page::CreditsRoll(_) => String::new(),
        _ if typing.field.is_some() => {
            if style == crate::pads::ButtonStyle::Keyboard {
                "Type the name   Backspace delete   Enter done".into()
            } else {
                typing::pad_keys(style)
            }
        }
        Page::PcSettings | Page::Layouts | Page::EditTeam(_) => style.menu_keys(true),
        _ => style.menu_keys(false),
    };
    let mut heading = title(page, &disc, &setup);
    // Rewards' close-up hides the screen's title (`+0x198`).
    if matches!(
        page,
        Page::Rewards | Page::RewardAsk | Page::DevelopmentRecipient(_)
    ) && menus.rewards.view.is_some()
    {
        heading.clear();
    }
    if let (Page::EventIntro(ev), Some(look)) = (page, fe.look.as_deref())
        && let Some(t) = look.text_id(crate::event_intro::title_id(ev))
    {
        heading = t.to_owned();
    }

    if let Some(look) = fe.look.as_deref() {
        let text = |at: Option<(&str, usize)>| at.and_then(|(f, i)| look.text(f, i));
        // A question over a panel keeps its screen's title.
        let titled = if matches!(page, Page::CabAsk(_)) {
            base
        } else {
            page
        };
        if let Some(t) = text(game_title(titled)) {
            heading = t.to_owned();
        }
        for (k, item) in list.iter_mut().enumerate() {
            if let Some(t) = text(game_label(page, k)) {
                *item = t.to_owned();
            }
        }
        let template = |i: usize| look.text("mapscreen.txt", i).map(str::to_owned);
        let help = if page == Page::Main && fe.idle.loading_demo(crate::attract::MAIN) {
            // "Loading Demo..." (`FUN_0029e980` on `+0xf0`: 0x406).
            look.text("main_help.txt", 6)
                .unwrap_or("Loading Demo...")
                .to_owned()
        } else if !message.0.is_empty() {
            message.0.clone()
        } else if matches!(page, Page::Bracket(_)) && fe.screen.is_some() {
            String::new()
        } else if matches!(page, Page::StoryCrew | Page::CreateTeam)
            && matches!(
                menus.crew_stage,
                crate::create_team::Stage::City
                    | crate::create_team::Stage::Name
                    | crate::create_team::Stage::Logo
                    | crate::create_team::Stage::Cancel
                    | crate::create_team::Stage::Save
            )
        {
            // `FUN_002810d0` empties the help (`FUN_0027fb88(.., "")`)
            // while its keyboards and Choose Logo are open.
            String::new()
        } else if page == Page::CreateTeam && menus.team_help_gone {
            // Nothing sets it again after the card's question closes.
            String::new()
        } else if page == Page::Layouts {
            options::layout_help(opts.0.layouts[menus.controller_player], menus.style)
        } else if matches!(base, Page::Trax) {
            crate::trax_view::rows(&opts.0)
                .get(base_cursor)
                .and_then(|&r| look.text("bigradio.txt", crate::trax_view::help(r)))
                .unwrap_or("")
                .to_owned()
        } else if matches!(
            page,
            Page::MoveSlots(_) | Page::MovePick(..) | Page::MoveAsk(..)
        ) {
            // Layout 85's own help line (element 1, `crate::edit_moves`).
            String::new()
        } else if matches!(base, Page::Rewards) {
            crate::rewards_view::help(look, &save.story, &disc, &menus.rewards)
        } else if page == Page::Story {
            // Element 46: "Select City." (0x1c0a).
            look.text_id(0x1c0a).unwrap_or("Select City.").to_owned()
        } else if let Page::City(c) = page
            && let Some(&ev) = story::city_rows(&save.story, c).get(cursor)
        {
            story::event_help(&save.story, &disc, ev, &template)
        } else if matches!(page, Page::Sides | Page::Users) {
            // `FUN_002979a8`: `main_help.txt` 14 for Pick Up games, 13 for
            // the other modes; "Enter cheat codes now." (`entername.txt`
            // 44) once every column is ready (`FUN_00298210`).
            let (file, index) = if page == Page::Users && all_ready(&menus, &setup) {
                ("entername.txt", 44)
            } else if menus.league_user_setup || menus.school_user_setup {
                ("main_help.txt", 13)
            } else {
                ("main_help.txt", 14)
            };
            text(Some((file, index)))
                .map(str::to_owned)
                .unwrap_or_else(|| own_help(page, cursor).to_owned())
        } else if matches!(page, Page::EventIntro(_) | Page::EditBaller(_)) {
            // (Layout 79 has no help line.)
            String::new()
        } else if matches!(base, Page::Careers) {
            let slots = crate::careers::slots(&save.story, &disc);
            slots
                .get(base_cursor)
                .map(|slot| crate::careers::help(look, slot, false))
                .unwrap_or_default()
        } else if base == Page::Ballers {
            let slots = crate::careers::baller_slots(&save.story, &disc);
            slots
                .get(base_cursor)
                .map(|slot| crate::careers::help(look, slot, true))
                .unwrap_or_default()
        } else if page == Page::League {
            crate::region_view::help(look, &disc, None)
        } else if let Page::LeagueRegion(r) = page {
            crate::region_view::help(
                look,
                &disc,
                street_sim::story::Story::league_opponent(r, cursor),
            )
        } else if page == Page::Online {
            // While a code or a name is typed, the line shows it, with the
            // cursor.
            match typing.field {
                Some(Field::Server) => format!(
                    "Code: {}_   Enter to join.",
                    menus.online.server.to_uppercase()
                ),
                Some(Field::PlayerName) => format!("Name: {}_", menus.online.name),
                _ => menus.online.help(cursor),
            }
        } else if matches!(page, Page::MyRules | Page::RulesCancelAsk) {
            // `FUN_002ca060`: the focused row's line, per mode for the
            // first three rows.
            let row = menus.under_dialog().map_or(cursor, |(_, c)| c);
            look.text_id(menus.rules_edit.help_id(row))
                .unwrap_or("")
                .to_owned()
        } else {
            // The game's line breaks (a backslash and n in its text) break
            // the help where they do in the original's kind-5 text.
            text(if matches!(page, Page::Delete(_) | Page::DataAsk) {
                game_help(base, base_cursor)
            } else {
                game_help(page, cursor)
            })
            .unwrap_or_else(|| own_help(page, cursor))
            .replace("\\n ", "\n")
            .replace("\\n", "\n")
        };
        // Under its cancel dialog, My Rules keeps its rows.
        let rules_rows: Vec<String> = if page == Page::RulesCancelAsk {
            items(
                Page::MyRules,
                &menus,
                &disc,
                &setup,
                &save,
                (&sound, &pads, &opts.0, sounds.songs()),
                &editing,
                &team_editing,
            )
        } else {
            Vec::new()
        };
        // Game Data's rows in the disc's words, under its dialogs too.
        let data_rows = if base == Page::GameData {
            crate::game_data::rows(look)
        } else {
            Vec::new()
        };
        // Front-end updates since the page opened (60 a second).
        let now = fe.time.elapsed_secs();
        if fe.opened.is_none_or(|(shown, _)| shown != page) {
            *fe.opened = Some((page, now));
        }
        let ticks = fe.opened.map_or(0, |(_, at)| ((now - at) * 60.0) as u64);
        let style = match base {
            Page::Title => Style::Title {
                demo: fe.idle.loading_demo(crate::attract::TITLE),
            },
            // The rewrite's Online page on the Main Menu's layout (six
            // rows, `online::ROWS`).
            Page::Main | Page::Online => Style::Screen(3),
            Page::PlayIt => Style::Screen(5),
            Page::Difficulty => Style::Screen(105),
            Page::Sides | Page::Users => Style::Screen(11),
            Page::ChooseTeam(_) | Page::LeagueTeam => Style::Screen(26),
            Page::NewCareer(_) | Page::PremadeAsk(_) | Page::Wizard(..) => Style::Screen(61),
            Page::EditBaller(_) | Page::MoveSlots(_) | Page::MovePick(..) | Page::TheMoves(_) => {
                Style::Screen(crate::cab_view::EDIT.class)
            }
            Page::Completed(mode) => Style::Screen(crate::completed_view::class_of(mode)),
            Page::JoinTeam => Style::Screen(crate::join_view::CLASS),
            // The brackets' layouts (classes 88, 89, 94) hold their title.
            Page::Bracket(ev) if fe.screen.is_some() => Style::Screen(
                crate::bracket_view::class_of(crate::bracket_view::shown(&save.story, ev).kind),
            ),
            Page::ChoosePlayers => Style::Screen(crate::choose::CLASS),
            Page::UpdateTeam(_) => Style::Screen(crate::update_view::CLASS),
            Page::StoryCrew | Page::CreateTeam => Style::Screen(crate::create_team::CLASS),
            Page::League | Page::LeagueRegion(_) => Style::Screen(crate::region_view::CLASS),
            Page::ChooseCourt | Page::PlayGameAsk => Style::Screen(33),
            Page::MyRules | Page::RulesCancelAsk => Style::Screen(39),
            Page::MyTeam => Style::Screen(6),
            Page::Options => Style::Screen(crate::options_view::CLASS),
            Page::HighScores => Style::Screen(crate::score_view::CLASS),
            Page::Trax => Style::Screen(crate::trax_view::CLASS),
            Page::Rewards => Style::Screen(crate::rewards_view::CLASS),
            Page::GameData => Style::Screen(crate::game_data::CLASS),
            Page::Inside => Style::Screen(102),
            Page::WhatsNew => Style::Screen(103),
            Page::Bobbito => Style::Screen(104),
            Page::Layouts => Style::Screen(55),
            Page::Story | Page::City(_) => Style::Screen(56),
            Page::Careers | Page::Ballers => Style::Screen(crate::careers::CLASS),
            Page::EventIntro(ev) => Style::Screen(crate::event_intro::class(ev)),
            Page::Credits => Style::Screen(u32::from(crate::credits::INTRO_CLASS)),
            Page::CreditsRoll(_) => Style::Screen(crate::credits::ROLL_CLASS),
            Page::Controls => Style::Read,
            _ => Style::List,
        };
        let view = View {
            style,
            title: &heading,
            warm: matches!(
                page,
                Page::Main
                    | Page::Title
                    | Page::Rewards
                    | Page::RewardAsk
                    | Page::DevelopmentRecipient(_)
            ),
            // Choose Players draws its own head-to-head panels.
            items: if matches!(
                page,
                Page::ChoosePlayers
                    | Page::UpdateTeam(_)
                    | Page::StoryCrew
                    | Page::CreateTeam
                    | Page::League
                    | Page::LeagueRegion(_)
                    | Page::Layouts
                    | Page::Users
                    | Page::Sides
                    | Page::ChooseTeam(_)
                    | Page::LeagueTeam
                    | Page::Completed(_)
                    | Page::JoinTeam
                    | Page::NewCareer(_)
                    | Page::PremadeAsk(_)
                    | Page::Wizard(..)
                    | Page::WizardAsk(_)
                    | Page::EditBaller(_)
                    | Page::MoveSlots(_)
                    | Page::MovePick(..)
                    | Page::MoveAsk(..)
                    | Page::Options
                    | Page::OptionsAsk
                    | Page::HighScores
                    | Page::Trax
                    | Page::TraxAsk
                    | Page::Rewards
                    | Page::RewardAsk
                    | Page::DevelopmentRecipient(_)
                    | Page::ChooseCourt
                    | Page::PlayGameAsk
                    | Page::Careers
                    | Page::CareerAsk(_)
                    | Page::BallerAsk(_)
            ) || (fe.screen.is_some()
                && matches!(page, Page::Story | Page::City(_) | Page::Bracket(_)))
            {
                &[]
            } else if page == Page::RulesCancelAsk {
                &rules_rows
            } else if base == Page::GameData {
                &data_rows
            } else {
                &list
            },
            // The lit row comes with the screen's state 4, after its
            // arrival (the rows arrive unlit).
            // Main's list goes unlit while it loads the demo (`FUN_0029e980`:
            // `FUN_00249368(.., 0xff, 0)`).
            cursor: if fe.transition.rows_lit()
                && !(page == Page::Main && fe.idle.loading_demo(crate::attract::MAIN))
            {
                base_cursor
            } else {
                usize::MAX
            },
            help: &help,
            keys: &keys,
            // Create A Baller: the rows on the backdrop's right panel, the
            // baller in its picture panel on the left.
            left: if matches!(page, Page::Inside | Page::WhatsNew | Page::Bobbito) {
                // The DVD pages' rows sit right of the backdrop's picture
                // (their layouts put them at x 352).
                352.0
            } else {
                look::LIST_X
            },
            ticks,
        };
        let mut pieces = look::build(&view, look, &mut fe.labels, &mut fe.images);
        // PC Settings: the project's own notice under its rows.
        if page == Page::PcSettings {
            pieces.extend(look::notes(
                look,
                &mut fe.labels,
                &mut fe.images,
                &options::NOTICE,
            ));
        }
        let class = screen_of(page, &menus, &save.story);
        if let (Some(image), Some(rect)) = (film.image(), movie_panel_rect(&page, class))
            && !matches!(
                page,
                Page::TheMoves(_)
                    | Page::Rewards
                    | Page::RewardAsk
                    | Page::DevelopmentRecipient(_)
                    | Page::MoveSlots(_)
                    | Page::MovePick(..)
                    | Page::MoveAsk(..)
            )
        {
            // The loop repaints the backdrop: not one of the screen's
            // widgets, it stays through their transitions.
            let piece = look::Piece {
                role: look::Role::Fixed,
                ..look::Piece::new(image, rect)
            };
            if movie_under_rows(&page, class) {
                pieces.insert(0, piece);
            } else {
                pieces.push(piece);
            }
        }
        // Create Team: the logo it wears, on the panel's right.
        if let (Page::EditTeam(_), Some(t), Some(art)) =
            (page, team_editing.team.as_ref(), art.as_ref())
            && let Some(logo) = art.logos.get(usize::from(t.logo))
        {
            pieces.push(look::Piece::new(
                logo.clone(),
                Rect::from_corners(
                    Vec2::new(456.0, 120.0),
                    Vec2::new(456.0, 120.0) + crate::art::LOGO_TILE * 2.0,
                ),
            ));
        }
        // Choose Players: the highlighted player's skills on the game's
        // sliders, named as `common.txt` names them.
        // Controller Layout: the pad and what the highlighted player's
        // layout puts on each button.
        // A credits roll: its lines rising, its skyline over them.
        if let (Page::CreditsRoll(_), Some(roll)) = (page, menus.credits.as_ref()) {
            pieces.extend(roll.pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art.as_ref().and_then(|a| a.credits_picture.as_ref()),
            ));
        }
        if let Some(screen) = fe.screen.as_deref() {
            pieces.extend(crate::bracket_view::page_pieces(
                page,
                cursor,
                look,
                &mut fe.labels,
                &mut fe.images,
                screen,
                &save.story,
                &disc,
            ));
        }
        if page == Page::Layouts {
            let layout = opts.0.layouts[menus.controller_player].min(2);
            pieces.extend(crate::diagram::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                menus.controller_player,
                menus.controller_view,
                cursor,
                pads.vibration,
            ));
        }
        if matches!(page, Page::Sides | Page::Users) {
            let typed = menus
                .naming
                .filter(|_| typing.field == Some(Field::UserName))
                .map(|seat| {
                    (
                        seat,
                        crate::users::Typed {
                            text: &menus.new_name,
                            key: typing.key,
                        },
                    )
                });
            let columns = user_columns(page, &menus, &setup, &save.story, typed.as_ref());
            let lit = (page == Page::Users).then_some(cursor);
            pieces.extend(crate::users::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                &columns,
                lit,
                ticks,
            ));
        }
        if let (Page::ChooseTeam(side), Some(art)) = (page, art.as_ref()) {
            let grid = crate::chooseteam::grid(&disc, &save.story);
            // The first seat on a side colours its panel.
            let player = |s: usize| setup.seats.iter().position(|&(_, x)| x == s);
            let panel = |s: usize| {
                if s == side {
                    crate::chooseteam::Panel::Picking {
                        cursor,
                        first: menus.team_scroll[side],
                        player: player(s),
                    }
                } else if let Some((seat, picked)) = menus.online_others[s] {
                    // Online, the other machine's side as it stands: live on
                    // its team until its picks are in, then "Ready" (its
                    // frame in its person's colour, 4P's past four).
                    if picked {
                        crate::chooseteam::Panel::Ready { player: Some(seat) }
                    } else {
                        let at = crate::chooseteam::position(&grid, setup.teams[s]);
                        crate::chooseteam::Panel::Picking {
                            cursor: at,
                            first: crate::chooseteam::scroll(0, at),
                            player: Some(seat),
                        }
                    }
                } else if s < side {
                    crate::chooseteam::Panel::Ready { player: player(s) }
                } else if player(s).is_some() {
                    // People on both sides (class 0x1c): the other side's
                    // panel is live too, its cursor on its team; the
                    // rewrite keeps a page per side, so it only looks it.
                    let grid = crate::chooseteam::grid(&disc, &save.story);
                    let at = crate::chooseteam::position(&grid, setup.teams[s]);
                    crate::chooseteam::Panel::Picking {
                        cursor: at,
                        first: crate::chooseteam::scroll(0, at),
                        player: player(s),
                    }
                } else {
                    crate::chooseteam::Panel::Cpu
                }
            };
            let sides = [0, 1].map(|s| crate::chooseteam::Side {
                team: setup.teams[s],
                panel: panel(s),
            });
            pieces.extend(crate::chooseteam::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                &grid,
                sides,
                ticks,
            ));
        }
        if let (Page::NewCareer(_), Some(cab), Some(gm)) =
            (base, fe.cab.as_deref(), fe.gm.as_deref())
            && let Some(layout) = look
                .layouts
                .iter()
                .find(|l| l.class == crate::cab_view::PREMADE)
        {
            let record = usize::from(street_sim::story::BALLER_RECORDS) + base_cursor;
            let values = crate::cab_view::premade_values(look, &disc, record);
            pieces.extend(crate::cab_view::panel_pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                cab,
                gm,
                layout,
                &values,
            ));
            let name = disc
                .players
                .get(record)
                .map(|p| p.name().to_uppercase())
                .unwrap_or_default();
            crate::cab_view::header(
                &mut pieces,
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                Some(&name),
                (true, true),
                crate::cab_view::CREATE,
                &[],
            );
            if let Page::PremadeAsk(_) = page {
                let words = look
                    .text("createplyr.txt", 14)
                    .unwrap_or("CREATE\\n%1S?")
                    .replace("%1S", &name);
                let rows: Vec<String> = [(12, "YES"), (11, "EDIT"), (13, "CANCEL")]
                    .iter()
                    .map(|&(k, or)| look.text("createplyr.txt", k).unwrap_or(or).to_owned())
                    .collect();
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        // `FUN_0025c970(.., 2)`.
                        row_font: 2,
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        // Create Baller's wizard: the step's panel (`cab_view.rs`), its
        // keyboard on the name panel, "CREATE %1S?" over the icons.
        if let (Page::Wizard(_, at), Some(cab), Some(gm), Some(b)) = (
            base,
            fe.cab.as_deref(),
            fe.gm.as_deref(),
            editing.baller.as_ref(),
        ) && let Some(layout) = look
            .layouts
            .iter()
            .find(|l| l.class == crate::cab_view::Step::at(at).layout())
        {
            let step = crate::cab_view::Step::at(at);
            let wizard = crate::cab_view::Wizard {
                baller: b,
                extras: &editing.outfit_extras,
                step,
                top: menus.wizard_top,
                skill: editing.skill,
                item: ballers::item_price(&editing, step),
                total: ballers::points_left(&editing, &save.story),
                skill_prices: ballers::skill_prices(b),
                icon_top: menus.wizard_icon_top,
                edit: None,
            };
            let values = crate::cab_view::wizard_values(
                look,
                layout,
                &wizard,
                fe.portraits.as_deref_mut(),
                &mut fe.images,
            );
            pieces.extend(crate::cab_view::panel_pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                cab,
                gm,
                layout,
                &values,
            ));
            crate::cab_view::header(
                &mut pieces,
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                None,
                step.marks(),
                crate::cab_view::CREATE,
                &[],
            );
            if matches!(
                step,
                crate::cab_view::Step::First | crate::cab_view::Step::Last
            ) {
                let typed = if step == crate::cab_view::Step::First {
                    &b.first_name
                } else {
                    &b.last_name
                };
                crate::cab_view::keyboard(
                    &mut pieces,
                    &mut fe.labels,
                    &mut fe.images,
                    look,
                    layout,
                    typed,
                    typing.key,
                );
            }
            if let Page::WizardAsk(_) = page {
                let name = format!("{} {}", b.first_name, b.last_name)
                    .trim()
                    .to_uppercase();
                let words = look
                    .text("createplyr.txt", 14)
                    .unwrap_or("CREATE\\n%1S?")
                    .replace("%1S", &name);
                let rows: Vec<String> = [(12, "YES"), (11, "EDIT"), (13, "CANCEL")]
                    .iter()
                    .map(|&(k, or)| look.text("createplyr.txt", k).unwrap_or(or).to_owned())
                    .collect();
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        // `FUN_0025c970(.., 2)`.
                        row_font: 2,
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        // Edit Baller: the panel (`cab_view::Edit`) on layout 79.
        if let (Page::EditBaller(_), Some(cab), Some(gm), Some(b)) = (
            base,
            fe.cab.as_deref(),
            fe.gm.as_deref(),
            editing.baller.as_ref(),
        ) {
            let edit = crate::cab_view::Edit::at(menus.edit_panel);
            if let Some(layout) = look.layouts.iter().find(|l| l.class == edit.layout()) {
                let wizard = crate::cab_view::Wizard {
                    baller: b,
                    extras: &editing.outfit_extras,
                    step: edit.step().unwrap_or(crate::cab_view::Step::Outfit),
                    top: menus.wizard_top,
                    skill: editing.skill,
                    item: ballers::edit_price(&editing, edit),
                    // The outfit or shoes under the cursor are only shown
                    // until bought (`FUN_0028e710`): Total leaves them out.
                    total: ballers::points_left(&editing, &save.story)
                        + match edit {
                            crate::cab_view::Edit::Outfit | crate::cab_view::Edit::Shoes => {
                                ballers::edit_price(&editing, edit).max(0)
                            }
                            _ => 0,
                        },
                    skill_prices: ballers::skill_prices(b),
                    icon_top: 0,
                    edit: Some(edit),
                };
                let values = crate::cab_view::wizard_values(
                    look,
                    layout,
                    &wizard,
                    fe.portraits.as_deref_mut(),
                    &mut fe.images,
                );
                pieces.extend(crate::cab_view::panel_pieces(
                    look,
                    &mut fe.labels,
                    &mut fe.images,
                    cab,
                    gm,
                    layout,
                    &values,
                ));
                crate::cab_view::header(
                    &mut pieces,
                    look,
                    &mut fe.labels,
                    &mut fe.images,
                    layout,
                    None,
                    (true, true),
                    crate::cab_view::EDIT,
                    &[crate::cab_view::EXIT],
                );
            }
        }
        // Game Data's question or notice: the card widget's own box.
        if let (Page::DataAsk, Some(gm), Some(ask)) =
            (page, fe.gm.as_deref(), menus.data_ask.as_ref())
        {
            pieces.extend(crate::card::box_pieces(
                look,
                gm,
                (&mut fe.labels, &mut fe.images, &mut fe.panels),
                &ask.words(look),
                None,
                &ask.rows(look),
                (cursor, crate::card::DIM, ticks),
            ));
        }
        // Game Data's delete list over its rows (`FUN_00258a48`).
        if let (Page::Delete(kind), Some(gm)) = (page, fe.gm.as_deref()) {
            let (words, rows) = (crate::game_data::list_title(look, kind), list.clone());
            pieces.extend(crate::pause::dialog(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                &crate::pause::Dialog {
                    text: &words,
                    rows: &rows,
                    row_font: 2,
                    lit: cursor,
                    ticks,
                    dim: crate::pause::FRONT_END_DIM,
                    width: 0.0,
                },
            ));
        }
        // Rewards: the screen's own widgets on layout 20, and its question.
        if matches!(base, Page::Rewards)
            && let (Some(layout), Some(art)) = (
                look.layouts
                    .iter()
                    .find(|l| l.class == crate::rewards_view::CLASS),
                art.as_ref(),
            )
        {
            pieces.extend(crate::rewards_view::pieces(
                look,
                art,
                &mut fe.labels,
                &mut fe.images,
                layout,
                &save.story,
                &disc,
                &menus.rewards,
                (
                    film.image(),
                    fe.gm.as_deref().map(|gm| &gm.white),
                    fe.big_card.0.as_ref().map(|(_, h, size)| {
                        let back = fe.turn.back.as_ref().filter(|_| fe.turn.face_back);
                        let face = back.map_or(h, |(_, b)| b);
                        (face.clone(), *size, fe.turn.squeeze)
                    }),
                ),
                ticks,
            ));
        }
        if let (Page::RewardAsk, Some(gm), Some(ask)) =
            (page, fe.gm.as_deref(), menus.reward_ask.as_ref())
        {
            let words = crate::pause::wrap(look, ask.words(), crate::rewards_view::ASK_WIDTH);
            let rows = ask.rows(look);
            pieces.extend(crate::pause::dialog(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                &crate::pause::Dialog {
                    text: &words,
                    rows: &rows,
                    row_font: 2,
                    lit: cursor,
                    ticks,
                    dim: crate::pause::FRONT_END_DIM,
                    width: crate::rewards_view::ASK_WIDTH,
                },
            ));
        }
        // The development points' recipient over the close-up
        // (`FUN_0024ed88`).
        if let (Page::DevelopmentRecipient(_), Some(gm)) = (page, fe.gm.as_deref()) {
            let names: Vec<String> = crate::rewards::development_recipients(&save.story)
                .iter()
                .map(|&r| disc.players[usize::from(r)].name())
                .collect();
            pieces.extend(crate::rewards_view::recipient_pieces(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                &crate::rewards_view::Recipient {
                    names: &names,
                    lit: cursor,
                    ticks,
                },
            ));
        }
        // Trax: the radio's rows, and its cancel question.
        if matches!(base, Page::Trax) {
            pieces.extend(crate::trax_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                &opts.0,
                base_cursor,
                menus.trax_top,
                ticks,
            ));
        }
        if let (Page::TraxAsk, Some(gm)) = (page, fe.gm.as_deref()) {
            let words = look
                .text("bigradio.txt", 47)
                .unwrap_or("Do you want to cancel\\nyour changes?")
                .to_owned();
            let rows: Vec<String> = [(2, "Yes"), (3, "No")]
                .iter()
                .map(|&(k, or)| look.text("common.txt", k).unwrap_or(or).to_owned())
                .collect();
            pieces.extend(crate::pause::dialog(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                &crate::pause::Dialog {
                    text: &words,
                    rows: &rows,
                    row_font: 2,
                    lit: cursor,
                    ticks,
                    dim: crate::pause::FRONT_END_DIM,
                    width: 0.0,
                },
            ));
        }
        // High Scores: the tab's table on layout 10.
        if page == Page::HighScores
            && let Some(layout) = look
                .layouts
                .iter()
                .find(|l| l.class == crate::score_view::CLASS)
        {
            pieces.extend(crate::score_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                &save.story.records,
                base_cursor,
                ticks,
            ));
        }
        // Options: its rows on layout 40, and its cancel question.
        if matches!(base, Page::Options)
            && let Some(layout) = look
                .layouts
                .iter()
                .find(|l| l.class == crate::options_view::CLASS)
        {
            pieces.extend(crate::options_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                &crate::options_view::Shown {
                    settings: &opts.0,
                    sound: &sound,
                    difficulty: setup.rules.difficulty,
                    cursor: base_cursor,
                    top: menus.options_top,
                    ticks,
                },
            ));
            if let (Page::OptionsAsk, Some(gm)) = (page, fe.gm.as_deref()) {
                let words = look
                    .text("options.txt", 46)
                    .unwrap_or("Do you want to cancel\\nyour changes?")
                    .to_owned();
                let rows: Vec<String> = [(2, "Yes"), (3, "No")]
                    .iter()
                    .map(|&(k, or)| look.text("common.txt", k).unwrap_or(or).to_owned())
                    .collect();
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        row_font: 2,
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        // A question over the wizard or the editor.
        if let (Page::CabAsk(_), Some(question), Some(gm)) =
            (page, menus.cab_ask.as_ref(), fe.gm.as_deref())
        {
            let (words, width) = question.words(look);
            let text = crate::pause::wrap(look, &words, width);
            let rows = question.rows(look);
            pieces.extend(crate::pause::dialog(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                &crate::pause::Dialog {
                    text: &text,
                    rows: &rows,
                    row_font: 2,
                    lit: cursor,
                    ticks,
                    dim: crate::pause::FRONT_END_DIM,
                    width,
                },
            ));
        }
        // Edit Moves: layout 85's panel on the editor's screen (layout 79),
        // the film room's clip over the backdrop's still, then its question.
        let nobody = street_sim::story::CreatedBaller::default();
        let browsing = matches!(base, Page::TheMoves(_));
        if let (
            Page::MoveSlots(cat) | Page::MovePick(cat, _) | Page::TheMoves(cat),
            Some(cab),
            Some(gm),
            Some(b),
        ) = (
            base,
            fe.cab.as_deref(),
            fe.gm.as_deref(),
            editing.baller.as_ref().or(browsing.then_some(&nobody)),
        ) && let Some(layout) = look
            .layouts
            .iter()
            .find(|l| l.class == crate::edit_moves::LAYOUT)
        {
            let (slot, pick) = match base {
                Page::MovePick(_, slot) => (slot, Some(base_cursor)),
                Page::TheMoves(_) => (0, Some(base_cursor)),
                _ => (base_cursor, None),
            };
            let shown = crate::edit_moves::Shown {
                disc: &disc,
                story: &save.story,
                baller: b,
                cat: crate::edit_moves::Category::at(cat),
                slot,
                pick,
                panel: &menus.moves,
                total: ballers::points_left(&editing, &save.story),
                browse: browsing,
            };
            let values = crate::edit_moves::values(look, &shown);
            let extra = crate::edit_moves::extra(
                look,
                &mut fe.labels,
                &mut fe.images,
                &gm.white,
                layout,
                &shown,
            );
            pieces.extend(extra.under);
            if let Some(clip) = film.image() {
                pieces.push(look::Piece::new(clip, crate::edit_moves::FILM_ROOM));
            }
            pieces.extend(crate::cab_view::panel_pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                cab,
                gm,
                layout,
                &values,
            ));
            pieces.extend(extra.over);
            // Step 1's "Exit" on Start, raised 3 (`FUN_002921f8`); step 2's
            // "Cancel" on Triangle (`FUN_00291c70`); The Moves' "Exit" on
            // Triangle alone and its "Preview Move" on Cross
            // (`FUN_00294658`).
            let pairs = match (browsing, pick) {
                (true, _) => vec![
                    crate::cab_view::LegendPair {
                        action: 1,
                        alone: true,
                        ..crate::cab_view::EXIT
                    },
                    crate::cab_view::LegendPair {
                        element: 15,
                        label: 0x3601,
                        action: 0,
                        icon_dy: 0.0,
                        alone: false,
                    },
                ],
                (false, None) => vec![crate::cab_view::LegendPair {
                    icon_dy: -3.0,
                    ..crate::cab_view::EXIT
                }],
                (false, Some(_)) => vec![crate::cab_view::LegendPair {
                    label: 5,
                    action: 1,
                    ..crate::cab_view::EXIT
                }],
            };
            crate::cab_view::header(
                &mut pieces,
                look,
                &mut fe.labels,
                &mut fe.images,
                layout,
                None,
                (false, false),
                crate::cab_view::EDIT,
                &pairs,
            );
            if let (Page::MoveAsk(..), Some(ask)) = (page, menus.moves.ask) {
                let (words, width) = ask.words(look, &disc);
                let text = crate::pause::wrap(look, &words, width);
                let rows = ask.rows(look);
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &text,
                        rows: &rows,
                        row_font: 2,
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width,
                    },
                ));
            }
        }
        if let Page::Completed(mode) = page {
            let star = save
                .story
                .crew
                .first()
                .and_then(|&r| disc.players.get(usize::from(r)));
            let streets: Vec<usize> = (0..street_sim::story::EVENTS.len())
                .filter(|&e| {
                    street_sim::story::EVENTS[e].kind
                        == street_sim::story::EventKind::StreetChallenge
                })
                .collect();
            let level = match setup.rules.difficulty {
                Difficulty::Easy => 0,
                Difficulty::Medium => 1,
                Difficulty::Hard => 2,
            };
            let board = crate::completed_view::Board {
                reward_points: street_sim::story::LEAGUE_DONE_RP[level],
                baller: star.map_or_else(Default::default, |p| {
                    (p.first_name.clone(), p.last_name.clone())
                }),
                wins: save.story.wins,
                losses: save.story.losses,
                reputation: save.story.reputation_level(),
                streets: (
                    streets.iter().filter(|&&e| save.story.cleared(e)).count(),
                    streets.len(),
                ),
            };
            pieces.extend(crate::completed_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                mode,
                &board,
            ));
        }
        if let (Page::LeagueTeam, Some(art)) = (page, art.as_ref()) {
            let grid = crate::chooseteam::league_grid(&disc, &save.story);
            let side = crate::chooseteam::Side {
                team: save.story.league_team.map_or(0, usize::from),
                panel: crate::chooseteam::Panel::Picking {
                    cursor,
                    first: menus.team_scroll[0],
                    player: Some(0),
                },
            };
            pieces.extend(crate::chooseteam::league_pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                &grid,
                side,
                ticks,
            ));
        }
        if let (Page::JoinTeam, Some(art)) = (page, art.as_ref()) {
            pieces.extend(crate::join_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                &menus.join,
                0,
                ticks,
            ));
        }
        if let (Page::ChooseCourt, Some(art)) = (base, art.as_ref()) {
            pieces.extend(crate::choosecourt::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                base_cursor,
                menus.court_random,
                |c| crate::choosecourt::open(&save.story, &disc, c),
                ticks,
            ));
        }
        if let (Page::EventIntro(ev), Some(art)) = (page, art.as_ref()) {
            pieces.extend(crate::event_intro::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                &save.story,
                ev,
            ));
        }
        if base == Page::Ballers {
            let slots = crate::careers::baller_slots(&save.story, &disc);
            pieces.extend(crate::careers::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                &slots,
                base_cursor,
                ticks,
            ));
        }
        if base == Page::Careers {
            let slots = crate::careers::slots(&save.story, &disc);
            pieces.extend(crate::careers::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                &slots,
                base_cursor,
                ticks,
            ));
        }
        if let (Page::BallerAsk(slot), Some(gm)) = (page, fe.gm.as_deref()) {
            let slots = crate::careers::baller_slots(&save.story, &disc);
            if let Some(s) = slots.get(slot) {
                let (words, rows) = crate::careers::dialog_rows(look, s, true);
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        // As Choose Baller's: 3 on an empty slot, else 2.
                        row_font: if matches!(s, crate::careers::Slot::Empty) {
                            3
                        } else {
                            2
                        },
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        if let (Page::CareerAsk(slot), Some(gm)) = (page, fe.gm.as_deref()) {
            let slots = crate::careers::slots(&save.story, &disc);
            if let Some(s) = slots.get(slot) {
                let (words, rows) = crate::careers::dialog_rows(look, s, false);
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        // `FUN_002cdf10`: 3 for an empty slot's dialog,
                        // 2 for a career's.
                        row_font: if save.story.career_at(slot).is_some() {
                            2
                        } else {
                            3
                        },
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        if base == Page::MyRules {
            // The Spot Points row names the people on each side (the
            // rewrite's side 0 is home), "CPU" for a side with nobody.
            let people: Vec<(usize, Option<String>)> = setup
                .seats
                .iter()
                .enumerate()
                .map(|(k, _)| (k, setup.users.get(k).cloned().flatten()))
                .collect();
            let side_of = |k: usize| setup.seats.get(k).is_some_and(|&(_, s)| s == 0);
            let away = crate::myrules::side_names(look, &people, false, side_of);
            let home = crate::myrules::side_names(look, &people, true, side_of);
            pieces.extend(crate::myrules::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                &menus.rules_edit,
                [&away, &home],
                base_cursor,
            ));
        }
        if matches!(page, Page::PlayGameAsk | Page::RulesCancelAsk) {
            pieces.extend(dialog_pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art.as_deref(),
                page,
                cursor,
                ticks,
            ));
        }
        if page == Page::ChoosePlayers {
            let side = |k: usize| {
                let picker = &menus.players.sides[k];
                let turn = picker.people.get(picker.turn).or(picker.people.first());
                crate::choose::SideLook {
                    picker,
                    controller: turn.map(|&d| seat_number(&setup, d)),
                    numbers: picker
                        .people
                        .iter()
                        .map(|&d| seat_number(&setup, d))
                        .collect(),
                }
            };
            pieces.extend(crate::choose::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                fe.portraits.as_deref_mut(),
                &disc,
                [side(0), side(1)],
                menus.players.mode,
                ticks,
            ));
        }
        if let (Page::League | Page::LeagueRegion(_), Some(art)) = (page, art.as_deref()) {
            let (r, focus) = match page {
                Page::LeagueRegion(r) => (r, Some(cursor)),
                _ => (
                    cursor.min(street_sim::story::REGION_MATCHES.len() - 1),
                    None,
                ),
            };
            let teams = street_sim::story::REGION_MATCHES[r];
            let (boss, court) = street_sim::story::REGION_REWARDS[r];
            pieces.extend(crate::region_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                art,
                &disc,
                &crate::region_view::View {
                    region: r,
                    teams,
                    boss,
                    court: usize::from(court),
                    beaten: (0..=teams.len())
                        .map(|k| save.story.league_done(r, k))
                        .collect(),
                    focus,
                },
                ticks,
            ));
        }
        if page == Page::CreateTeam {
            let cats = team_categories(&disc, &save.story);
            if let Some(category) = cats.get(menus.team_pick.category) {
                pieces.extend(crate::create_team::pieces(
                    look,
                    &mut fe.labels,
                    &mut fe.images,
                    fe.portraits.as_deref_mut(),
                    &disc,
                    &crate::create_team::View {
                        mode: crate::create_team::Mode::MyTeam,
                        category,
                        picker: &menus.team_pick,
                        baller: None,
                    },
                    ticks,
                ));
            }
        }
        if page == Page::StoryCrew {
            let category = crate::create_team::Category {
                title: 6,
                players: menus.locals.clone(),
                rows: Vec::new(),
            };
            pieces.extend(crate::create_team::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                fe.portraits.as_deref_mut(),
                &disc,
                &crate::create_team::View {
                    mode: crate::create_team::Mode::Crew,
                    category: &category,
                    picker: &menus.crew_pick,
                    baller: save.story.crew.first().map(|&r| usize::from(r)),
                },
                ticks,
            ));
        }
        // The boards and questions over Create Team's grid
        // (`FUN_002810d0`'s dialogs), the crew's and My Team's.
        if matches!(page, Page::StoryCrew | Page::CreateTeam)
            && let Some(gm) = fe.gm.as_deref()
        {
            use crate::create_team::Stage;
            match menus.crew_stage {
                Stage::Welcome => pieces.extend(crate::create_team::welcome_pieces(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    ticks,
                )),
                Stage::City | Stage::Name => {
                    let city = menus.crew_stage == Stage::City;
                    let heading = look
                        .text("createteam.txt", if city { 16 } else { 17 })
                        .unwrap_or(if city {
                            "Where's Your Team From?"
                        } else {
                            "What's Your Team's Name?"
                        })
                        .to_owned();
                    let typed = if city {
                        &menus.crew_city
                    } else {
                        &menus.crew_name
                    };
                    pieces.extend(crate::create_team::keyboard_pieces(
                        look,
                        gm,
                        &mut fe.labels,
                        &mut fe.images,
                        &mut fe.panels,
                        &heading,
                        typed,
                        typing.key,
                    ));
                }
                Stage::Logo => {
                    if let Some(art) = art.as_ref() {
                        pieces.extend(crate::create_team::logo_pieces(
                            look,
                            gm,
                            &mut fe.labels,
                            &mut fe.images,
                            &mut fe.panels,
                            art,
                            menus.crew_logo,
                            ticks,
                        ));
                    }
                }
                Stage::Save => {
                    let whole = format!("{} {}", menus.crew_city, menus.crew_name);
                    pieces.extend(crate::card::box_pieces(
                        look,
                        gm,
                        (&mut fe.labels, &mut fe.images, &mut fe.panels),
                        &menus.team_card.words(look, &whole),
                        None,
                        &menus.team_card.rows(look),
                        (menus.team_ask, crate::card::DIM, ticks),
                    ));
                }
                Stage::Cancel => {
                    let mode = if page == Page::StoryCrew {
                        crate::create_team::Mode::Crew
                    } else {
                        crate::create_team::Mode::MyTeam
                    };
                    if let Some((words, rows)) =
                        crate::create_team::question(look, menus.crew_stage, mode)
                    {
                        pieces.extend(crate::pause::dialog(
                            look,
                            gm,
                            &mut fe.labels,
                            &mut fe.images,
                            &mut fe.panels,
                            &crate::pause::Dialog {
                                text: &words,
                                rows: &rows,
                                row_font: 2,
                                lit: menus.team_ask,
                                ticks,
                                dim: crate::pause::FRONT_END_DIM,
                                width: 0.0,
                            },
                        ));
                    }
                }
                Stage::Pick => {}
            }
        }
        if let (Page::UpdateTeam(stage), Some(u)) = (page, menus.update.as_ref()) {
            let crew: Vec<usize> = save.story.crew.iter().map(|&r| usize::from(r)).collect();
            let beaten: Vec<usize> = u.beaten.iter().map(|&r| usize::from(r)).collect();
            let beaten_at = u
                .pick
                .and_then(|p| u.beaten.iter().position(|&b| b == p))
                .unwrap_or(0);
            let (focus, roster_at, beaten_at) = match stage {
                UpdateStage::Pick => (crate::update_view::Focus::Beaten, 0, cursor),
                UpdateStage::Slot => (crate::update_view::Focus::Roster, cursor, beaten_at),
                UpdateStage::Accept => (crate::update_view::Focus::Roster, u.slot, beaten_at),
                _ => (crate::update_view::Focus::None, 0, beaten_at),
            };
            pieces.extend(crate::update_view::pieces(
                look,
                &mut fe.labels,
                &mut fe.images,
                fe.portraits.as_deref_mut(),
                &disc,
                &crate::update_view::View {
                    roster: &crew,
                    beaten: &beaten,
                    focus,
                    roster_at,
                    beaten_at,
                },
                ticks,
            ));
            // The question and the confirmations (`FUN_0025d328`: Yes, No).
            if let (UpdateStage::Ask | UpdateStage::Accept | UpdateStage::Exit, Some(gm)) =
                (stage, fe.gm.as_deref())
            {
                let words = look
                    .text(
                        "updateteam.txt",
                        match stage {
                            UpdateStage::Ask => 4,
                            UpdateStage::Accept => 2,
                            _ => 1,
                        },
                    )
                    .unwrap_or("")
                    .trim_end()
                    .to_owned();
                let rows = [
                    look.text("common.txt", 2).unwrap_or("Yes").to_owned(),
                    look.text("common.txt", 3).unwrap_or("No").to_owned(),
                ];
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        row_font: 2,
                        lit: cursor,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: 0.0,
                    },
                ));
            }
        }
        let shown = match page {
            Page::StoryTeam => save.story.crew.get(cursor).map(|&r| usize::from(r)),
            _ => None,
        };
        if let Some(record) = shown {
            let p = &disc.players[record];
            let rows: Vec<(String, u8)> = ballers::SKILLS
                .iter()
                .enumerate()
                .map(|(k, &(name, skill))| {
                    let label = look.text("common.txt", 10 + k).unwrap_or(name).to_owned();
                    let level = ((p.ratings[skill].clamp(0, 99) + 5) / 10) as u8;
                    (label, level)
                })
                .collect();
            pieces.extend(look::bars(
                look,
                &mut fe.labels,
                &mut fe.images,
                // Inside the backdrop's right-hand frame.
                Vec2::new(362.0, 100.0),
                190.0,
                &rows,
            ));
        }
        // The screen arriving after its transition, and the page as it
        // stands for the next screen's leaving.
        fe.transition
            .arrive(&mut pieces, fe.time.elapsed_secs_f64());
        fe.transition.keep(&pieces);
        // "Join a team"'s message (after its arrival, `+0x16c`) and
        // question, over it.
        if let (Page::JoinTeam, Some(gm)) = (page, fe.gm.as_deref()) {
            let join = &menus.join;
            let (words, rows, lit) = if join.message {
                let cont = look.text("common.txt", 20).unwrap_or("Continue");
                (crate::join_view::message(look), vec![cont.to_owned()], 0)
            } else if let Some(lit) = join.ask {
                (
                    crate::join_view::question(look, &disc, &save.story, join),
                    vec![
                        look.text("common.txt", 2).unwrap_or("Yes").to_owned(),
                        look.text("common.txt", 3).unwrap_or("No").to_owned(),
                    ],
                    lit,
                )
            } else {
                (String::new(), Vec::new(), 0)
            };
            if !rows.is_empty() {
                let words = crate::pause::wrap(look, &words, crate::join_view::WIDTH);
                pieces.extend(crate::pause::dialog(
                    look,
                    gm,
                    &mut fe.labels,
                    &mut fe.images,
                    &mut fe.panels,
                    &crate::pause::Dialog {
                        text: &words,
                        rows: &rows,
                        row_font: 2,
                        lit,
                        ticks,
                        dim: crate::pause::FRONT_END_DIM,
                        width: crate::join_view::WIDTH,
                    },
                ));
            }
        }
        // Select's question over the screen (`FUN_0024bfa0`).
        if let (Some(lit), Some(gm)) = (menus.exit_ask, fe.gm.as_deref()) {
            pieces.extend(crate::exit_ask::pieces(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                lit,
                ticks,
            ));
        }
        // Square held: the Help overlay over the screen (`FUN_00250000`).
        if menus.help
            && let (Some(class), Some(gm)) = (
                screen_class(page, &menus, &save.story, &disc),
                fe.gm.as_deref(),
            )
        {
            pieces.extend(crate::help_view::pieces(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                class,
            ));
        }
        // The memory card's dialog over the screen it works for.
        if let (Some(flow), Some(gm)) = (fe.card.flow.as_ref(), fe.gm.as_deref()) {
            pieces.extend(crate::card::pieces(
                look,
                gm,
                &mut fe.labels,
                &mut fe.images,
                &mut fe.panels,
                flow,
                crate::card::DIM,
                ticks,
            ));
        }
        look::show(&mut fe.commands, &mut fe.slots, &pieces, Screen::Menu);
        for mut t in title_text
            .iter_mut()
            .chain(body_text.iter_mut())
            .chain(hint_text.iter_mut())
        {
            t.0.clear();
        }
        return;
    }

    // Without the disc's fonts: plain text.
    look::show(&mut fe.commands, &mut fe.slots, &[], Screen::Menu);
    let window = 14;
    let first = cursor
        .saturating_sub(window / 2)
        .min(list.len().saturating_sub(window));
    let mut body: Vec<String> = list
        .iter()
        .enumerate()
        .skip(first)
        .take(window)
        .map(|(k, item)| {
            let static_page = matches!(page, Page::Controls | Page::Credits);
            if k == cursor && !(static_page && k + 1 < list.len()) {
                format!("> {item}")
            } else {
                format!("  {item}")
            }
        })
        .collect();
    if page == Page::Title {
        body.insert(0, "Streetball Revival\n".into());
    }
    if !message.0.is_empty() {
        body.push(String::new());
        body.push(message.0.clone());
    }
    if let Some(lit) = menus.exit_ask {
        body.push(String::new());
        body.push("Exit to Main Menu?".into());
        for (k, row) in ["Yes", "No"].iter().enumerate() {
            body.push(format!("{} {row}", if k == lit { ">" } else { " " }));
        }
    }
    for mut t in &mut title_text {
        t.0 = heading.clone();
    }
    for mut t in &mut body_text {
        t.0 = body.join("\n");
    }
    for mut t in &mut hint_text {
        t.0 = keys.clone();
    }
}

/// The two dialogs over Choose Court and My Rules. The base dialog
/// (`FUN_0025beb0`, `FUN_00260a68`) dims the screen with a 660 by 500
/// black rectangle at alpha 0x4f (`FUN_0024f508`'s first child,
/// `FUN_00233e00(.., 0x4f000000)`); the Play Game dialog's picture is
/// `XAFEBGEX` entry 6 ("My_r"), its rows "Play Game" (0xd06) and "My
/// Rules" (0xd03) font 3 centred at (402, 298) and (402, 332)
/// (`DAT_004cf4a8`, `DAT_004cf4b8`) with the row widget's stars. Where the
/// picture lands is the dialog layer's doing, not traced: the rewrite puts
/// its painted part where the original's grab shows it. The cancel
/// dialog's panel (`FUN_0025c860`'s kind-0x1c widget) is the rewrite's own
/// drawing of the original's: a navy panel with a blue edge, the question
/// (0xe14) font 2 centred, Yes and No rows font 3.
fn dialog_pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: Option<&art::Art>,
    page: Page,
    cursor: usize,
    ticks: u64,
) -> Vec<look::Piece> {
    use look::{Align, Piece, RowSpot, fonts};
    let mut out = Vec::new();
    let mut dim = Piece::new(
        Handle::default(),
        Rect::from_center_size(Vec2::new(320.0, 240.0), Vec2::new(660.0, 500.0)),
    );
    dim.colour = Color::srgba(0.0, 0.0, 0.0, look::black_alpha(79.0 / 255.0));
    out.push(dim);
    let mut stars = Vec::new();
    #[allow(clippy::too_many_arguments)]
    fn row(
        out: &mut Vec<Piece>,
        stars: &mut Vec<Piece>,
        labels: &mut look::Labels,
        images: &mut Assets<Image>,
        look: &Look,
        at: Vec2,
        text: &str,
        lit: bool,
        ticks: u64,
    ) {
        look::screen_row(
            out,
            stars,
            labels,
            look,
            images,
            RowSpot {
                at,
                font: 3,
                align: Align::Centre,
                valign: 0,
            },
            text,
            lit,
            true,
            ticks,
        );
    }
    match page {
        Page::PlayGameAsk => {
            if let Some((picture, size)) = art.and_then(|a| a.rules_dialog.as_ref()) {
                // The painted part spans (112, 94) to (541, 374) in the
                // original's grab: 1.85 times its texture.
                let at = Vec2::new(112.0, 94.0);
                out.push(Piece::new(
                    picture.clone(),
                    Rect::from_corners(at, at + *size * 1.85),
                ));
            }
            let play = look.text_id(0x0d06).unwrap_or("Play Game");
            let rules = look.text_id(0x0d03).unwrap_or("My Rules");
            row(
                &mut out,
                &mut stars,
                labels,
                images,
                look,
                Vec2::new(402.0, 298.0),
                play,
                cursor == 0,
                ticks,
            );
            row(
                &mut out,
                &mut stars,
                labels,
                images,
                look,
                Vec2::new(402.0, 332.0),
                rules,
                cursor != 0,
                ticks,
            );
        }
        Page::RulesCancelAsk => {
            let mut edge = Piece::new(Handle::default(), Rect::new(156.0, 94.0, 488.0, 364.0));
            edge.colour = Color::srgb_u8(0x3f, 0x4f, 0xb0);
            out.push(edge);
            let mut panel = Piece::new(Handle::default(), Rect::new(159.0, 97.0, 485.0, 361.0));
            panel.colour = Color::srgb_u8(0x0c, 0x12, 0x3c);
            out.push(panel);
            let question = look
                .text_id(0x0e14)
                .unwrap_or("Cancel Changes and\\nReturn to Choose Court?")
                .replace("\\n", "\n");
            let line = look
                .fonts
                .get(fonts::LIST)
                .map_or(27.0, |f| f.line_height as f32);
            for (k, l) in question.lines().enumerate() {
                if let Some(p) = labels.text(
                    look,
                    images,
                    fonts::LIST,
                    l,
                    look::colours::PALE,
                    None,
                    Vec2::new(322.0, 180.0 + line * k as f32),
                    Align::Centre,
                ) {
                    out.push(p);
                }
            }
            let yes = look.text("common.txt", 2).unwrap_or("Yes");
            let no = look.text("common.txt", 3).unwrap_or("No");
            row(
                &mut out,
                &mut stars,
                labels,
                images,
                look,
                Vec2::new(322.0, 246.0),
                yes,
                cursor == 0,
                ticks,
            );
            row(
                &mut out,
                &mut stars,
                labels,
                images,
                look,
                Vec2::new(322.0, 276.0),
                no,
                cursor != 0,
                ticks,
            );
        }
        _ => {}
    }
    out.extend(stars);
    out
}

/// Choose Team's page for `side`, its cursor on `team`.
impl Menus {
    /// Online, `side`'s team and three as another machine picked them:
    /// into the setup, and onto Choose Players' picker for that side if it
    /// is open (the computer's look: its three picked).
    pub(crate) fn mirror_side(
        &mut self,
        pick: crate::lobby::SidePick,
        setup: &mut MatchSetup,
        disc: &Disc,
        story: &street_sim::story::Story,
    ) {
        use crate::choose::{Mode, Picker, tiles};
        let side = pick.side;
        setup.teams[side] = pick.team;
        setup.lineups[side] = pick.lineup;
        if self.top().0 != Page::ChoosePlayers {
            return;
        }
        let roster: Vec<usize> = disc
            .teams
            .get(pick.team)
            .map(|t| t.roster.clone())
            .unwrap_or_default();
        let locked = |r: usize| {
            crate::choose::OLD_SCHOOL.contains(&r)
                && !story
                    .unlocked_players
                    .contains(&u16::try_from(r).unwrap_or(u16::MAX))
        };
        let t = tiles(
            pick.team,
            &roster,
            pick.lineup,
            false,
            Mode::PickUp,
            &locked,
        );
        let panel = self.players.panel(side);
        self.players.sides[panel] = Picker::computer(t, pick.lineup);
    }

    /// Open the menus on what this machine picks for the lobby's match
    /// (`lobby.rs`): its first side's Choose Team, or the creator's court
    /// when it picks no side. The page under it is the Online page, where
    /// backing out returns to the lobby (a match that ends goes back to the
    /// Main Menu under it).
    pub(crate) fn begin_online_pick(
        &mut self,
        duties: crate::lobby::Duties,
        setup: &mut MatchSetup,
        disc: &Disc,
        story: &street_sim::story::Story,
    ) {
        self.stack = vec![(Page::Title, 0), (Page::Main, 0), (Page::Online, 0)];
        self.online_others = [None; 2];
        let first = duties.sides.first().copied();
        self.online_pick = Some(duties);
        match first {
            Some(side) => push_choose_team(self, disc, story, setup.teams[side], side),
            None => {
                self.court_random = None;
                self.push(Page::ChooseCourt, crate::choosecourt::FIRST);
            }
        }
    }
}

fn push_choose_team(
    menus: &mut Menus,
    disc: &Disc,
    story: &street_sim::story::Story,
    team: usize,
    side: usize,
) {
    let at = crate::chooseteam::position(&crate::chooseteam::grid(disc, story), team);
    menus.team_scroll[side] = crate::chooseteam::scroll(0, at);
    menus.push(Page::ChooseTeam(side), at);
}

/// The people who pick a match side's players: its seats, 1P first, two at
/// most (`FUN_002322a8` keeps `+0xba` and `+0xbb`); with nobody seated one
/// person on home (any controller, which [`players_navigate`] reads as the
/// keyboard); nobody in a game the computer plays for both.
fn players_people(setup: &MatchSetup, side: usize) -> Vec<Device> {
    if setup.demo {
        Vec::new()
    } else if setup.seats.is_empty() {
        if side == 0 {
            vec![Device::Keyboard]
        } else {
            Vec::new()
        }
    } else {
        setup
            .seats
            .iter()
            .filter(|&&(_, s)| s == side)
            .map(|&(d, _)| d)
            .take(2)
            .collect()
    }
}

/// A controller's number (0 for 1P): its seat, in the order they joined.
fn seat_number(setup: &MatchSetup, device: Device) -> usize {
    setup
        .seats
        .iter()
        .position(|&(d, _)| d == device)
        .unwrap_or(0)
}

/// Choose Players' two pickers for the match `setup` describes
/// (`FUN_00269c08`, Pick Up's mode). A side nobody holds shows its three
/// picked: the line-up Choose Team's commit drew for it
/// ([`computer_three`]), or, when its players are not its team's (a match
/// set up without Choose Team), `FUN_001dbe30`'s draw from the match's
/// seed.
fn begin_players(
    menus: &mut Menus,
    disc: &Disc,
    setup: &mut MatchSetup,
    story: &street_sim::story::Story,
) {
    use crate::choose::{Mode, Picker, Screen, tiles};
    let mode = Mode::PickUp;
    let mut screen = Screen {
        mode,
        ..Default::default()
    };
    for side in 0..2 {
        let team = setup.teams[side];
        let roster: Vec<usize> = disc
            .teams
            .get(team)
            .map(|t| t.roster.clone())
            .unwrap_or_default();
        let people = players_people(setup, side);
        let held = !people.is_empty();
        let own = |r: &usize| match team {
            38 => crate::choose::OLD_SCHOOL.contains(r) || roster.contains(r),
            39 => crate::choose::LEAGUE_STARS.contains(r) || roster.contains(r),
            _ => roster.contains(r),
        };
        if !held && !setup.lineups[side].iter().all(own) {
            let book = crate::lineup::Book::of(disc);
            let unlocked = crate::lineup::unlocked(disc, story);
            let mode = crate::lineup::Mode::PickUp {
                unlocked: &unlocked,
            };
            let mut libc = crate::session::Libc::seeded(setup.seed);
            setup.lineups[side] =
                crate::lineup::choose(&book, &mode, team, [None; 3], None, &mut libc);
        }
        let locked = |r: usize| {
            crate::choose::OLD_SCHOOL.contains(&r)
                && !story
                    .unlocked_players
                    .contains(&u16::try_from(r).unwrap_or(u16::MAX))
        };
        let t = tiles(team, &roster, setup.lineups[side], held, mode, &locked);
        screen.sides[screen.panel(side)] = if held {
            Picker::held(t, people, mode)
        } else {
            Picker::computer(t, setup.lineups[side])
        };
    }
    menus.players = screen;
}

/// A new career's Create Team (class 0x62, `FUN_0027dd80(screen, 1)`): the
/// ten local ballers (drawn once a run from the session's C library
/// generator, as `FUN_001ed1f8` draws them from the C library's), the grid
/// at its start, no picks.
fn begin_crew(
    menus: &mut Menus,
    disc: &Disc,
    story: &street_sim::story::Story,
    session: Option<&crate::session::Session>,
) {
    if menus.locals.is_empty() {
        let mut libc = session.map_or(crate::session::Libc::seeded(0), |s| s.libc());
        let crew: Vec<usize> = story.crew.iter().map(|&r| usize::from(r)).collect();
        menus.locals = crate::create_team::locals(disc, &mut libc, &|r| crew.contains(&r));
    }
    menus.crew = story.crew.iter().take(1).copied().collect();
    menus.crew_pick = Default::default();
    // The Welcome board first (`FUN_002810d0`'s state 5).
    menus.crew_stage = crate::create_team::Stage::Welcome;
    menus.crew_logo = 0;
}

/// The crew's presses (`FUN_002810d0`'s states): Cross closes the
/// Welcome board; then the grid steps, Cross puts the focused local in the
/// next open slot (a pick already made only sounds), Triangle takes the
/// last back; the second pick opens the city's keyboard ("NEW YORK" to
/// start, `createteam.txt` 19), its end the name's ("BALLERS", 21), its end
/// Choose Logo (`FUN_00280cd0`); Cross there makes the crew and its team
/// (`FUN_002808c0`: the career's team record, the career's bit 0) and
/// opens the map, Triangle goes back to the name's keyboard.
fn crew_navigate(
    input: &MenuInput,
    menus: &mut Menus,
    save: &mut story::Save,
    typing: &mut Typing,
    look: Option<&Look>,
) {
    use crate::create_team::{Mode, Stage};
    let text = |k: usize, or: &str| {
        look.and_then(|l| l.text("createteam.txt", k))
            .unwrap_or(or)
            .to_owned()
    };
    let type_into = |typing: &mut Typing, field: Field| {
        typing.field = Some(field);
        // The keyboard opens on DONE (the default typed already).
        typing.key = (crate::typing::NAME_KEYBOARD.len() - 1, 0);
    };
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    match menus.crew_stage {
        Stage::Welcome => {
            if input.confirm() {
                menus.crew_stage = Stage::Pick;
            }
        }
        Stage::Pick => {
            let len = menus.locals.len();
            if dx != 0 || dy != 0 {
                menus.crew_pick.step(dx, dy, len);
            }
            if input.confirm() {
                let at = menus.crew_pick.at();
                if let Some(&record) = menus.locals.get(at)
                    && menus.crew_pick.pick(record, Mode::Crew)
                    && menus.crew_pick.picks.len() == Mode::Crew.picks()
                {
                    menus.crew_stage = Stage::City;
                    menus.crew_city = text(19, "NEW YORK");
                    type_into(typing, Field::CrewCity);
                }
            } else if input.back() && !menus.crew_pick.undo() {
                // Nothing to take back: "Cancel and Return to Choose
                // Baller?" (`createteam.txt` 22).
                menus.crew_stage = Stage::Cancel;
                menus.team_ask = crate::create_team::QUESTION_ROW;
            }
        }
        // A keyboard finished (`FUN_00280cd0`'s 0x3ed, 0x3ee).
        Stage::City => {
            menus.crew_stage = Stage::Name;
            menus.crew_name = text(21, "BALLERS");
            type_into(typing, Field::CrewName);
        }
        Stage::Name => menus.crew_stage = Stage::Logo,
        Stage::Logo => {
            if dx != 0 || dy != 0 {
                menus.crew_logo = crate::create_team::logo_step(menus.crew_logo, dx, dy);
            }
            if input.confirm() {
                let baller = save.story.crew.first().copied();
                let mut crew: Vec<u16> = baller.into_iter().collect();
                crew.extend(menus.crew_pick.picks.iter().map(|&r| r as u16));
                if crew.len() == CREW_SIZE {
                    save.story.crew = crew;
                    save.story.crew_team = street_sim::story::CrewTeam {
                        city: menus.crew_city.clone(),
                        name: menus.crew_name.clone(),
                        logo: crate::create_team::LOGOS.start + menus.crew_logo.min(9) as u8,
                    };
                    save.store();
                    menus.crew = save.story.crew.clone();
                    menus.stack.pop();
                    menus.save_to_card();
                }
            } else if input.back() {
                // Cancelled (`FUN_00280f98`'s 0x3ef): the name's keyboard
                // again, which then goes on to Choose Logo.
                menus.crew_stage = Stage::Name;
                type_into(typing, Field::CrewName);
            }
        }
        // Yes goes back to Choose Baller (`FUN_00280cd0`'s 0x3f2: class
        // 0x36) without the crew; No stays on the grid.
        Stage::Cancel => match question_keys(input, &mut menus.team_ask, 2) {
            Some(0) => {
                menus.crew_stage = Stage::Pick;
                // The map the crew was opened over, then Choose Baller.
                menus.stack.pop();
                if menus.top().0 == Page::Story {
                    menus.stack.pop();
                }
            }
            Some(_) => menus.crew_stage = Stage::Pick,
            None => {}
        },
        Stage::Save => menus.crew_stage = Stage::Logo,
    }
}

/// The card widget closed over My Team's Create Team without the team
/// saved: the grid again, its last pick taken back (`FUN_002808c0`:
/// `+0xfc` back two, `FUN_00280040`).
fn team_card_closed(menus: &mut Menus) {
    menus.team_pick.undo();
    menus.crew_stage = crate::create_team::Stage::Pick;
    menus.team_help_gone = true;
}

/// A Create Team question's keys: up and down between its rows, Cross
/// answers with the row, Triangle with No (1).
fn question_keys(input: &MenuInput, row: &mut usize, rows: usize) -> Option<usize> {
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dy != 0 {
        *row = (*row as i32 + dy).clamp(0, rows as i32 - 1) as usize;
        return None;
    }
    if input.confirm() {
        return Some(*row);
    }
    input.back().then_some(1)
}

/// "Join a team"'s keys ([`crate::join_view`]): Cross takes the arrival's
/// message away; on the grid the directions step it (`FUN_0026ed50`'s
/// list) and Cross asks (No lit); the question's Yes puts the baller on
/// the team and gives the next its turn, after the last the card's save
/// and Main; No (or Triangle) closes it.
fn join_keys(input: &MenuInput, menus: &mut Menus, save: &mut story::Save) {
    if menus.join.message {
        if input.confirm() {
            menus.join.message = false;
        }
        return;
    }
    let grid = crate::chooseteam::join_grid();
    if let Some(mut row) = menus.join.ask {
        match question_keys(input, &mut row, 2) {
            Some(0) => {
                let team = grid.get(menus.join.cursor).and_then(|c| c.team);
                if let (Some(team), Some(record)) = (team, menus.join.baller()) {
                    save.story.join_team(team as u16, record as u16);
                    save.store();
                    menus.teams_changed = true;
                }
                menus.join.ask = None;
                menus.join.at += 1;
                if menus.join.baller().is_none() {
                    menus.stack.truncate(2);
                    menus.save_to_card();
                }
            }
            Some(_) => menus.join.ask = None,
            None => menus.join.ask = Some(row),
        }
        return;
    }
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dx != 0 || dy != 0 {
        let join = &mut menus.join;
        join.cursor = crate::chooseteam::step(&grid, join.cursor, dx, dy);
        join.first = crate::chooseteam::scroll(join.first, join.cursor);
        return;
    }
    if input.confirm()
        && grid
            .get(menus.join.cursor)
            .is_some_and(|c| c.team.is_some())
    {
        menus.join.ask = Some(1);
    }
}

/// My Team's Create Team (class 0x35, `FUN_0027dd80(screen, 0)`): the
/// grid steps, L1 and R1 turn the category, Cross puts the focused player
/// in the next open slot, Triangle takes the last back (none: "Cancel and
/// Return to My Team?", `createteam.txt` 23). The fifth pick opens the
/// city's keyboard ("NEW YORK", 20), then the name's ("BALLERS", 21) and
/// Choose Logo, as the crew's (`FUN_002810d0`); then the memory card
/// widget's "Create team ...?" ([`crate::create_team::CardAsk`]): YES
/// makes the team (over the one of its name after "Overwrite existing
/// ...?"; with the ten slots used, "Only 10 teams ..." and the grid) and
/// saves to the card, then back to My Team (class 6); NO closes it on the
/// grid with the last pick taken back. Measured in the runtime: YES with
/// Autosave on, Checking 2.07 s, Saving 4.42, "Save successful." 2.02,
/// My Team.
fn team_navigate(
    input: &MenuInput,
    menus: &mut Menus,
    team_editing: &mut teams::TeamEditing,
    save: &mut story::Save,
    disc: &Disc,
    typing: &mut Typing,
    look: Option<&Look>,
) {
    use crate::create_team::{Mode, Stage};
    let text = |k: usize, or: &str| {
        look.and_then(|l| l.text("createteam.txt", k))
            .unwrap_or(or)
            .to_owned()
    };
    let type_into = |typing: &mut Typing, field: Field| {
        typing.field = Some(field);
        // The keyboard opens on DONE (the default typed already).
        typing.key = (crate::typing::NAME_KEYBOARD.len() - 1, 0);
    };
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    match menus.crew_stage {
        Stage::City => {
            menus.crew_stage = Stage::Name;
            menus.crew_name = text(21, "BALLERS");
            type_into(typing, Field::CrewName);
            return;
        }
        Stage::Name => {
            menus.crew_stage = Stage::Logo;
            return;
        }
        Stage::Logo => {
            if dx != 0 || dy != 0 {
                menus.crew_logo = crate::create_team::logo_step(menus.crew_logo, dx, dy);
            } else if input.confirm() {
                // `FUN_00280cd0`'s 0x3ef: the card widget's "Create team
                // ...?" (`FUN_002808c0`, `FUN_002272e8`).
                menus.crew_stage = Stage::Save;
                menus.team_card = crate::create_team::CardAsk::Create;
                menus.team_ask = menus.team_card.first_row();
            } else if input.back() {
                // `FUN_00280f98`'s 0x3ef sets the help line again.
                menus.team_help_gone = false;
                menus.crew_stage = Stage::Name;
                type_into(typing, Field::CrewName);
            }
            return;
        }
        Stage::Save => {
            // The card's dialog: up and down between its rows (they do not
            // wrap), Cross answers; Triangle does nothing (`FUN_00259380`).
            use crate::create_team::{CardAsk, CardNext};
            let rows = if menus.team_card == CardAsk::Full {
                1
            } else {
                2
            };
            if dy != 0 {
                menus.team_ask = (menus.team_ask as i32 + dy).clamp(0, rows - 1) as usize;
                return;
            }
            if !input.confirm() {
                return;
            }
            let whole = format!("{} {}", menus.crew_city, menus.crew_name);
            match menus
                .team_card
                .answer(menus.team_ask, &save.story.teams, &whole)
            {
                CardNext::Ask(ask) => {
                    menus.team_card = ask;
                    menus.team_ask = ask.first_row();
                }
                CardNext::Make(slot) => {
                    let mut roster = [0u16; street_sim::story::TEAM_PLAYERS];
                    for (row, &r) in menus.team_pick.picks.iter().enumerate() {
                        if let Some(at) = roster.get_mut(row) {
                            *at = r as u16;
                        }
                    }
                    let team = street_sim::story::CreatedTeam {
                        city: menus.crew_city.clone(),
                        name: menus.crew_name.clone(),
                        logo: crate::create_team::LOGOS.start + menus.crew_logo.min(9) as u8,
                        roster,
                    };
                    match slot.and_then(|k| save.story.teams.get_mut(k)) {
                        Some(old) => *old = team,
                        None => save.story.teams.push(team),
                    }
                    save.store();
                    team_editing.team = None;
                    // The card's save (0x3f) over Create Team's grid;
                    // saved, the screen goes back to My Team
                    // (`FUN_00280b60`: `FUN_002d7d40(.., 6)`).
                    menus.crew_stage = Stage::Pick;
                    menus.team_saving = true;
                    menus.stack.pop();
                    menus.save_to_card();
                }
                CardNext::Close => team_card_closed(menus),
            }
            return;
        }
        Stage::Cancel => {
            match question_keys(input, &mut menus.team_ask, 2) {
                Some(0) => {
                    team_editing.team = None;
                    menus.stack.pop();
                }
                Some(_) => menus.crew_stage = Stage::Pick,
                None => {}
            }
            return;
        }
        Stage::Welcome | Stage::Pick => {}
    }
    let cats = team_categories(disc, &save.story);
    let turn = input.shoulder();
    if turn != 0 {
        menus.team_pick.turn(turn, &cats);
    }
    let list = cats
        .get(menus.team_pick.category)
        .map(|c| c.players.clone())
        .unwrap_or_default();
    let dx = i32::from(input.right()) - i32::from(input.left());
    let dy = i32::from(input.down()) - i32::from(input.up());
    if dx != 0 || dy != 0 {
        menus.team_pick.step(dx, dy, list.len());
    }
    if input.confirm() {
        if let Some(&record) = list.get(menus.team_pick.at())
            && menus.team_pick.pick(record, Mode::MyTeam)
            && menus.team_pick.picks.len() == Mode::MyTeam.picks()
        {
            menus.crew_stage = Stage::City;
            menus.crew_city = text(20, "NEW YORK");
            type_into(typing, Field::CrewCity);
        }
    } else if input.back() && !menus.team_pick.undo() {
        menus.crew_stage = Stage::Cancel;
        menus.team_ask = crate::create_team::QUESTION_ROW;
    }
}

/// My Team's Create Team categories (`crate::create_team::categories`).
fn team_categories(
    disc: &Disc,
    story: &street_sim::story::Story,
) -> Vec<crate::create_team::Category> {
    let mut created: Vec<usize> = story.ballers.iter().map(|b| b.record()).collect();
    created.sort_unstable();
    crate::create_team::categories(
        disc,
        &|r| story.unlocked_players.contains(&(r as u16)),
        &created,
    )
}

/// What a frame of Choose Players' presses came to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlayersOutcome {
    Picking,
    /// Both sides finished and a Cross pressed: the line-ups are set.
    Finished,
    /// Triangle with nothing to take back.
    Left,
}

/// Choose Players' presses (`FUN_00230e98` on the picker of the person
/// whose turn it is, then the screen's `FUN_0026a860`): each controller
/// drives its own side's picker, directions moving and Cross picking (or,
/// ready, continuing); Triangle takes the last pick back or, with none,
/// goes back; a Cross once both sides are finished goes on (in the same
/// press as the last pick, as the picker answers first).
fn players_navigate(
    input: &MenuInput,
    menus: &mut Menus,
    setup: &mut MatchSetup,
    devices: &[Device],
) -> PlayersOutcome {
    use crate::choose::Press;
    let mode = menus.players.mode;
    let anyone = setup.seats.is_empty();
    let (mut go_on, mut leave) = (false, false);
    for &d in devices {
        let who = if anyone { Device::Keyboard } else { d };
        let (dx, dy) = (input.step_of(d), input.vertical_of(d));
        let (confirm, back) = (input.confirm_of(d), input.back_of(d));
        if let Some(p) = menus.players.sides.iter_mut().find(|p| p.takes(who)) {
            if dx != 0 {
                p.step(dx, 0);
            } else if dy != 0 {
                p.step(0, dy);
            }
            if confirm {
                p.pick(mode);
            }
            if back && p.undo(mode) == Press::Leave {
                leave = true;
            }
        }
        go_on |= confirm;
    }
    if leave {
        return PlayersOutcome::Left;
    }
    if go_on && menus.players.finished() {
        for side in 0..2 {
            if let Some(lineup) = menus.players.side(side).lineup() {
                setup.lineups[side] = lineup;
            }
        }
        return PlayersOutcome::Finished;
    }
    PlayersOutcome::Picking
}

/// Be A Legend's Choose Players (mode 5) for the game `story::prepare` set
/// up: the career's crew on the left, its baller picked from the start, up
/// to five places (the empty ones dark); the opponents' three on the right.
/// The League Challenge's (mode 4) for `story::prepare_league`'s: the
/// challenge's team on the left, its roster for the person to pick three
/// from (`FUN_00231b40`), the league opponent's three on the right.
fn begin_story_players(
    menus: &mut Menus,
    disc: &Disc,
    setup: &MatchSetup,
    story: &street_sim::story::Story,
    mode: crate::choose::Mode,
) {
    use crate::choose::{Mode, Picker, Screen, tiles};
    use crate::story::{CAREER_SIDE as MINE, OPPONENT_SIDE as THEIRS};
    let crew: Vec<usize> = if mode == Mode::Legend {
        story.crew.iter().map(|&r| usize::from(r)).collect()
    } else {
        setup.lineups[MINE].to_vec()
    };
    // The person's team on the left (the setup's slot 0, game side 1):
    // the away side's panel.
    let mut screen = Screen {
        mode,
        home_left: false,
        ..Default::default()
    };
    let home = if mode == Mode::Legend {
        tiles(
            setup.teams[MINE],
            &crew,
            setup.lineups[MINE],
            true,
            mode,
            &|_| false,
        )
    } else {
        let roster = disc
            .teams
            .get(setup.teams[MINE])
            .map(|t| t.roster.clone())
            .unwrap_or(crew);
        tiles(
            setup.teams[MINE],
            &roster,
            setup.lineups[MINE],
            true,
            mode,
            &|_| false,
        )
    };
    let away = tiles(
        setup.teams[THEIRS],
        &[],
        setup.lineups[THEIRS],
        false,
        mode,
        &|_| false,
    );
    screen.sides[screen.panel(MINE)] = Picker::held(home, vec![Device::Keyboard], mode);
    screen.sides[screen.panel(THEIRS)] = Picker::computer(away, setup.lineups[THEIRS]);
    menus.players = screen;
}

/// User Setup's name states for the seats chosen: every column asks
/// "Enter Name" first (`FUN_00245b88`, state 10's X).
fn begin_names(menus: &mut Menus, setup: &mut MatchSetup) {
    let n = setup.seats.len();
    setup.users.resize(n, None);
    menus.user_stages = vec![UserStage::START; n];
    menus.push(Page::Users, 0);
}

/// Whether every column is ready (`FUN_00298210`: then each shows
/// "Continue" and the help line changes).
fn all_ready(menus: &Menus, setup: &MatchSetup) -> bool {
    !setup.seats.is_empty()
        && menus.user_stages.len() >= setup.seats.len()
        && menus.user_stages[..setup.seats.len()]
            .iter()
            .all(|&s| s == UserStage::Ready)
}

/// The saved names seat `k` may pick: every user's, except names the
/// other seats hold.
fn saved_names(story: &story::Story, setup: &MatchSetup, k: usize) -> Vec<String> {
    let taken: Vec<&String> = setup
        .users
        .iter()
        .enumerate()
        .filter(|&(j, u)| j != k && u.is_some())
        .filter_map(|(_, u)| u.as_ref())
        .collect();
    story
        .records
        .users
        .iter()
        .map(|u| u.name.clone())
        .filter(|n| !taken.contains(&n))
        .collect()
}

/// The keyboard opens for seat `k`, its cursor on DONE over the default
/// name (`FUN_0023c7b8`).
fn begin_typing(menus: &mut Menus, typing: &mut Typing, k: usize) {
    menus.naming = Some(k);
    menus.new_name.clear();
    typing.field = Some(Field::UserName);
    typing.key = (crate::typing::NAME_KEYBOARD.len() - 1, 0);
    if let Some(s) = menus.user_stages.get_mut(k) {
        *s = UserStage::Typing;
    }
}

/// User Setup's keys: the highlighted column answers up, down, confirm and
/// back as the original's column widget does (`FUN_00245b88`); left and
/// right move between columns (the rewrite's own, where the original's
/// columns each answer only their controller).
#[allow(clippy::too_many_arguments)]
fn users_navigate(
    input: &MenuInput,
    menus: &mut Menus,
    setup: &mut MatchSetup,
    save: &mut story::Save,
    disc: &Disc,
    typing: &mut Typing,
    _message: &mut Message,
    cheats: &mut crate::cheats::Settings,
) {
    let n = setup.seats.len();
    if setup.users.len() < n {
        setup.users.resize(n, None);
    }
    if menus.user_stages.len() != n {
        menus.user_stages.resize(n, UserStage::START);
    }
    let cursor = menus.top().1.min(n.saturating_sub(1));
    *menus.cursor() = cursor;
    let step = i32::from(input.right()) - i32::from(input.left());
    if step != 0 && n > 1 {
        *menus.cursor() = (cursor as i32 + step).rem_euclid(n as i32) as usize;
        return;
    }
    let Some(stage) = menus.user_stages.get(cursor).copied() else {
        if input.back() {
            menus.stack.pop();
        }
        return;
    };
    let names = saved_names(&save.story, setup, cursor);
    let (up, down, confirm, back) = (input.up(), input.down(), input.confirm(), input.back());
    let ready_now = all_ready(menus, setup);
    match stage {
        UserStage::Ask { yes } => {
            if up || down {
                menus.user_stages[cursor] = UserStage::Ask { yes: !yes };
            }
            if confirm {
                if !yes {
                    setup.users[cursor] = None;
                    menus.user_stages[cursor] = UserStage::Guest;
                } else if names.is_empty() {
                    begin_typing(menus, typing, cursor);
                } else {
                    menus.user_stages[cursor] = UserStage::Pick { at: 0, first: 0 };
                }
            } else if back {
                // Back to the side chooser (the original's state 10).
                cheats.reset();
                menus.league_user_setup = false;
                menus.school_user_setup = false;
                menus.stack.pop();
            }
        }
        UserStage::Pick { at, first } => {
            let rows = names.len() + 1;
            let mut at = at;
            if up && at > 0 {
                at -= 1;
            }
            if down && at + 1 < rows {
                at += 1;
            }
            let first = if at < first {
                at
            } else if at >= first + crate::users::at::LIST_ROWS {
                at + 1 - crate::users::at::LIST_ROWS
            } else {
                first
            };
            menus.user_stages[cursor] = UserStage::Pick { at, first };
            if confirm {
                if at == 0 {
                    begin_typing(menus, typing, cursor);
                } else {
                    setup.users[cursor] = Some(names[at - 1].clone());
                    menus.user_stages[cursor] = UserStage::Named;
                }
            } else if back {
                menus.user_stages[cursor] = UserStage::START;
            }
        }
        UserStage::Typing => {}
        UserStage::Named | UserStage::Guest => {
            if confirm {
                menus.user_stages[cursor] = UserStage::Ready;
                // On to the next column still to answer.
                if let Some(next) = (0..n).find(|&k| menus.user_stages[k] != UserStage::Ready) {
                    *menus.cursor() = next;
                }
            } else if back {
                setup.users[cursor] = None;
                menus.user_stages[cursor] = UserStage::START;
            }
        }
        UserStage::Ready => {
            if confirm && ready_now {
                after_user_setup(menus, setup, save, disc);
            } else if back {
                menus.user_stages[cursor] = if setup.users[cursor].is_some() {
                    UserStage::Named
                } else {
                    UserStage::Guest
                };
            }
        }
    }
}

/// Where User Setup goes (`FUN_00297c40`'s next class): Street School's
/// lessons, the League Challenge's players, or Choose Team.
fn after_user_setup(
    menus: &mut Menus,
    setup: &mut MatchSetup,
    save: &mut story::Save,
    disc: &Disc,
) {
    if menus.school_user_setup {
        menus.school_user_setup = false;
        crate::school::begin_for_setup(&mut save.story, setup);
        *menus.stack.last_mut().unwrap() = (Page::School, 0);
        return;
    }
    if menus.league_user_setup {
        menus.league_user_setup = false;
        menus.school_user_setup = false;
        story::begin_league_for_setup(&mut save.story, setup);
        // Class 0x19 opens on its list's first node (`FUN_0021f230`).
        menus.team_scroll[0] = 0;
        *menus.stack.last_mut().unwrap() = (Page::LeagueTeam, 0);
        return;
    }
    setup.demo = setup.seats.is_empty();
    push_choose_team(menus, disc, &save.story, setup.teams[0], 0);
}

/// The four columns as the page shows them: on Choose Sides every
/// controller's side chooser (a plugged controller that has not picked
/// reads "Press START"), on the name page each seat's state; empty ports
/// past them.
fn user_columns<'a>(
    page: Page,
    menus: &'a Menus,
    setup: &'a MatchSetup,
    story: &'a story::Story,
    typed: Option<&'a (usize, crate::users::Typed<'a>)>,
) -> Vec<crate::users::Column<'a>> {
    use crate::users::Column;
    let go = all_ready(menus, setup);
    (0..crate::users::COLUMNS.len())
        .map(|k| {
            if page == Page::Sides {
                let Some(&device) = menus.devices.get(k) else {
                    return Column::Empty { port: k + 1 };
                };
                match setup.seats.iter().find(|&&(d, _)| d == device) {
                    Some(&(_, side)) => Column::Sides { side: Some(side) },
                    None if menus.joined.contains(&device) => Column::Sides { side: None },
                    None => Column::Idle,
                }
            } else {
                let Some(&(_, side)) = setup.seats.get(k) else {
                    return Column::Empty { port: k + 1 };
                };
                let stage = menus
                    .user_stages
                    .get(k)
                    .copied()
                    .unwrap_or(UserStage::START);
                match stage {
                    UserStage::Ask { yes } => Column::Ask { yes },
                    UserStage::Pick { at, first } => Column::Pick {
                        names: saved_names(story, setup, k),
                        at,
                        first,
                    },
                    UserStage::Typing => match typed {
                        Some((seat, t)) if *seat == k => Column::Typing(t),
                        _ => Column::Ask { yes: true },
                    },
                    UserStage::Named => setup
                        .users
                        .get(k)
                        .cloned()
                        .flatten()
                        .and_then(|n| story.records.user(&n))
                        .and_then(|i| story.records.users.get(i))
                        .map_or(Column::Guest, Column::Named),
                    UserStage::Guest => Column::Guest,
                    UserStage::Ready => Column::Ready { side, go },
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_new_careers_baller_takes_a_star_slot_of_its_own() {
        use street_sim::story::{BALLER_RECORDS, BALLER_SLOTS, CreatedBaller, STAR_SLOTS, Story};
        let mut s = Story::default();
        // Past My Team's six first, as the original's careers keep theirs.
        assert_eq!(career_star_slot(&s), Some(BALLER_SLOTS));
        s.ballers.push(CreatedBaller {
            slot: BALLER_SLOTS as u8,
            ..Default::default()
        });
        assert_eq!(career_star_slot(&s), Some(BALLER_SLOTS + 1));
        // A career whose star is a slot's record holds it too.
        s.start_career(0, BALLER_RECORDS + BALLER_SLOTS as u16 + 1);
        assert_eq!(career_star_slot(&s), Some(BALLER_SLOTS + 2));
        for slot in BALLER_SLOTS..STAR_SLOTS {
            s.ballers.push(CreatedBaller {
                slot: slot as u8,
                ..Default::default()
            });
        }
        assert_eq!(career_star_slot(&s), Some(0));
    }

    #[test]
    fn the_wizards_name_panel_steps_with_its_keyboards() {
        use crate::cab_view::Step;
        let mut menus = Menus::default();
        menus.push(Page::Wizard(3, Step::First.index()), 0);
        menus.wizard_name_step(Field::CabFirst, true);
        assert_eq!(menus.top().0, Page::Wizard(3, Step::Last.index()));
        menus.wizard_name_step(Field::CabLast, true);
        assert_eq!(menus.top().0, Page::Wizard(3, Step::Icon.index()));
        menus.push(Page::Wizard(3, Step::Last.index()), 0);
        menus.wizard_name_step(Field::CabLast, false);
        assert_eq!(menus.top().0, Page::Wizard(3, Step::First.index()));
        menus.wizard_name_step(Field::CabFirst, false);
        assert_eq!(menus.top().0, Page::Wizard(3, Step::Skills.index()));
        // The dialog sits over the panel.
        menus.push(Page::WizardAsk(3), 2);
        assert_eq!(
            menus.under_dialog().map(|(p, _)| p),
            Some(Page::Wizard(3, Step::Skills.index()))
        );
        assert_eq!(preview_framing(&menus), crate::cab_view::Framing::Body);
    }

    /// The School's game started and a lesson launched from its menu.
    fn start_lesson(
        lesson: usize,
        save: &mut story::Save,
        disc: &Disc,
        setup: &mut MatchSetup,
        running: &mut story::Running,
        message: &mut Message,
    ) {
        start_school(disc, setup, running, message);
        if running.0.is_some() {
            crate::school::start_for_setup(&mut save.story, setup, lesson);
            running.0 = Some(story::Run::School(lesson));
        }
    }

    #[test]
    fn school_production_entry_attempt_replay_result_and_payout_keep_the_owner() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut save = story::Save {
            path: std::env::temp_dir().join("street-school-profile-unused.dat"),
            story: street_sim::story::Story::default(),
            last_error: None,
        };
        save.story.records.add_user("A").unwrap();
        save.story.records.add_user("B").unwrap();
        let pad = Device::Pad(Entity::from_raw_u32(7).unwrap());
        let mut setup = MatchSetup::new(&disc, 0, 1);
        setup.seats = vec![(pad, 1)];
        setup.users = vec![Some("A".into())];
        seat_league_user(&mut setup, pad);
        crate::school::begin_for_setup(&mut save.story, &setup);
        let mut running = story::Running::default();
        let mut message = Message(String::new());
        for passed in [false, true, false] {
            start_lesson(0, &mut save, &disc, &mut setup, &mut running, &mut message);
            assert!(matches!(running.0, Some(story::Run::School(0))));
            // The person plays the Lakers, side 1 (`FUN_001db968`).
            assert_eq!(setup.seats, [(pad, 1)]);
            assert_eq!(setup.teams, [crate::school::NETS, crate::school::LAKERS]);
            assert_eq!(setup.users, [Some("A".into())]);
            assert!(crate::school::finish_for_setup(
                &mut save.story,
                &setup,
                0,
                passed
            ));
        }
        assert_eq!(save.story.school_profile.attempts[0], 3);
        assert_eq!(save.story.school_profile.challenges[0], 12);
        setup.users = vec![Some("B".into())];
        crate::school::begin_for_setup(&mut save.story, &setup);
        assert_eq!(save.story.school, 0);
        for lesson in 0..26 {
            start_lesson(
                lesson,
                &mut save,
                &disc,
                &mut setup,
                &mut running,
                &mut message,
            );
            crate::school::finish_for_setup(&mut save.story, &setup, lesson, true);
        }
        assert!(
            crate::school::completion_rewards_for_setup(&mut save.story, &setup)
                .contains(&street_sim::story::Notice::RewardPoints(1000))
        );
        assert!(crate::school::completion_rewards_for_setup(&mut save.story, &setup).is_empty());
        assert_eq!(save.story.reward_points, 1000);
        setup.users = vec![Some("A".into())];
        crate::school::begin_for_setup(&mut save.story, &setup);
        assert_eq!(save.story.school, 1);
        assert_eq!(save.story.school_profile.challenges[0], 12);
        setup.users = vec![None];
        crate::school::begin_for_setup(&mut save.story, &setup);
        start_lesson(2, &mut save, &disc, &mut setup, &mut running, &mut message);
        crate::school::finish_for_setup(&mut save.story, &setup, 2, true);
        let mut loaded = street_sim::story::Story::read(&save.story.write()).unwrap();
        assert_eq!(loaded.school, 0);
        assert!(loaded.school_trophy);
        crate::school::begin_for_setup(&mut loaded, &setup);
        assert_eq!(loaded.school_profile.attempts, [0; 26]);
        setup.online = true;
        assert!(!crate::school::start_for_setup(&mut loaded, &setup, 0));
        assert!(!crate::school::finish_for_setup(
            &mut loaded,
            &setup,
            0,
            true
        ));
        assert!(crate::school::completion_rewards_for_setup(&mut loaded, &setup).is_empty());
    }

    #[test]
    fn league_production_entry_setup_and_postgame_keep_profile_and_guest_isolation() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut save = story::Save {
            path: std::env::temp_dir().join("street-league-profile-unused.dat"),
            story: street_sim::story::Story::default(),
            last_error: None,
        };
        save.story.records.add_user("A").unwrap();
        save.story.records.add_user("B").unwrap();
        let mut setup = MatchSetup::new(&disc, 0, 1);
        setup.seats = vec![(Device::Keyboard, 0)];
        let mut running = story::Running::default();
        let mut message = Message(String::new());
        let won = street_sim::story::MatchOutcome {
            won: true,
            ..Default::default()
        };
        for (name, region, index) in [(Some("A"), 0, 0), (Some("B"), 1, 0), (None, 4, 0)] {
            setup.users = vec![name.map(str::to_owned)];
            seat_league_user(&mut setup, Device::Keyboard);
            // The exact helper called by the production User Setup Continue.
            story::begin_league_for_setup(&mut save.story, &setup);
            assert_eq!(save.story.league_played, 0);
            // Choose Team (class 0x19) stores the challenge's team.
            save.story.league_team = Some(16);
            running.0 = None;
            start_league_match(
                region,
                index,
                &mut save,
                &disc,
                &mut setup,
                &mut running,
                &mut message,
            );
            assert!(
                matches!(running.0, Some(story::Run::League(r, i)) if r == region && i == index)
            );
            assert_eq!(setup.users, [name.map(str::to_owned)]);
            assert_eq!(setup.seats, [(Device::Keyboard, story::CAREER_SIDE)]);
            assert!(story::league_selected_user_eligible(&setup));
            // The exact helper called by game.rs after a completed league match.
            assert!(
                !story::play_league_for_setup(&mut save.story, &setup, region, index, &won)
                    .is_empty()
            );
            assert!(save.story.league_done(region, index));
        }
        let profiles = save
            .story
            .records
            .users
            .iter()
            .map(|u| u.league.clone())
            .collect::<Vec<_>>();
        assert_eq!(profiles[0].played, 1);
        assert_eq!(profiles[1].played, 1 << 8);
        let mut loaded = street_sim::story::Story::read(&save.story.write()).unwrap();
        assert_eq!(
            loaded.league_played, 0,
            "guest progress never becomes the unassigned save"
        );
        setup.users = vec![Some("A".into())];
        story::begin_league_for_setup(&mut loaded, &setup);
        assert_eq!(loaded.league_progress(), profiles[0]);
        setup.online = true;
        let before = loaded.league_progress();
        assert!(story::play_league_for_setup(&mut loaded, &setup, 0, 1, &won).is_empty());
        assert_eq!(loaded.league_progress(), before);
        setup.online = false;
        setup.users = vec![None];
        story::begin_league_for_setup(&mut loaded, &setup);
        assert_eq!(loaded.league_played, 0);
        assert!(loaded.league_roster.is_empty());
    }

    #[test]
    fn league_seats_the_selected_person_and_records_their_local_win_and_loss() {
        let pad = Device::Pad(Entity::from_raw_u32(7).unwrap());
        let mut setup = MatchSetup {
            teams: [0, 1],
            lineups: [[0, 1, 2], [3, 4, 5]],
            court: "RUCKER".into(),
            rules: Rules::default(),
            seed: 1,
            demo: false,
            watch: false,
            seats: vec![(Device::Keyboard, 0), (pad, 1)],
            users: vec![Some("A".into()), Some("B".into())],
            online: true,
        };
        let mut progress = street_sim::story::Story {
            games: 17,
            wins: 8,
            losses: 9,
            ..Default::default()
        };
        progress.records.add_user("A").unwrap();
        progress.records.add_user("B").unwrap();
        seat_league_user(&mut setup, pad);
        assert_eq!(setup.seats, [(pad, story::CAREER_SIDE)]);
        assert_eq!(setup.users, [Some("B".into())]);
        assert!(story::league_selected_user_eligible(&setup));
        let hoops = std::array::from_fn(|side| street_sim::Hoop {
            rim: [if side == 0 { 37.0 } else { -37.0 }, 11.75, 0.0],
            rim_radius: 0.75,
        });
        let players = (0..6)
            .map(|index| street_sim::Player::new(index / 3, [0.0, index as f32], 0.0))
            .collect();
        let mut world = street_sim::World::new(hoops, players, setup.rules, 1);
        // The person's team plays side 1 (`story::CAREER_SIDE`).
        world.score = [15, 21];
        let mut stats = crate::game::Stats::new(6);
        stats.dunks[3] = 2;
        for winner in [0, 1] {
            progress.play_league(
                0,
                0,
                &street_sim::story::MatchOutcome {
                    won: winner == story::CAREER_SIDE,
                    ..Default::default()
                },
            );
            crate::highscores::record_match(&mut progress, &world, &stats, &setup, winner);
        }
        let selected = &progress.records.users[1];
        assert_eq!(
            (
                selected.wins,
                selected.losses,
                selected.career[0],
                selected.career[2]
            ),
            (1, 1, 42, 4)
        );
        assert_eq!(
            (
                progress.records.users[0].wins,
                progress.records.users[0].losses
            ),
            (0, 0)
        );
        assert_eq!((progress.games, progress.wins, progress.losses), (17, 8, 9));
        assert_eq!(
            street_sim::story::Story::read(&progress.write()).unwrap(),
            progress
        );
        setup.demo = true;
        assert!(!story::league_selected_user_eligible(&setup));
        crate::highscores::record_match(&mut progress, &world, &stats, &setup, 0);
        assert_eq!(progress.records.users[1].wins, 1);
        setup.demo = false;
        setup.online = true;
        assert!(!story::league_selected_user_eligible(&setup));
        setup.online = false;
        setup.seats.clear();
        assert!(!story::league_selected_user_eligible(&setup));
        setup.seats = vec![(pad, story::OPPONENT_SIDE)];
        assert!(!story::league_selected_user_eligible(&setup));
        setup.seats.push((Device::Keyboard, story::CAREER_SIDE));
        assert!(!story::league_selected_user_eligible(&setup));
        seat_league_user(&mut setup, Device::Keyboard);
        assert_eq!(setup.users, [None]);
        assert!(
            story::league_selected_user_eligible(&setup),
            "explicit local guest can play"
        );
    }

    /// Controllers step between Away, CPU and Home; joining takes the next
    /// number while one is free, and leaving frees it.
    #[test]
    fn difficulty_leads_on_by_the_play_it_row() {
        // `FUN_002bf428`: Pick Up Game, League Challenge and Be A Legend all go
        // through Difficulty (class 0x69); `FUN_00284848` then goes on by
        // the mode.
        let row = |arm: usize| PLAY_IT_ROWS.iter().position(|&a| a == arm).unwrap();
        assert_eq!(
            after_difficulty(Some((Page::PlayIt, row(3)))),
            AfterDifficulty::ChooseBaller
        );
        assert_eq!(
            after_difficulty(Some((Page::PlayIt, row(2)))),
            AfterDifficulty::LeagueUserSetup
        );
        assert_eq!(
            after_difficulty(Some((Page::PlayIt, row(0)))),
            AfterDifficulty::ChooseSides
        );
        assert_eq!(row(3), PLAY_IT_LEGEND);
    }

    #[test]
    fn the_career_dialog_rows_do_what_fun_002cd9c8_does() {
        for row in 0..3 {
            assert_eq!(career_choice(false, row), CareerChoice::NewBaller);
        }
        assert_eq!(career_choice(false, 3), CareerChoice::Cancel);
        assert_eq!(career_choice(true, 0), CareerChoice::Continue);
        assert_eq!(career_choice(true, 1), CareerChoice::EditBaller);
        assert_eq!(career_choice(true, 2), CareerChoice::EditMoves);
        assert_eq!(career_choice(true, 3), CareerChoice::Cancel);
    }

    #[test]
    fn choose_players_seats_its_pickers_as_fun_00269c08_builds_them() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let story = street_sim::story::Story::default();
        let mut setup = MatchSetup::new(&disc, 0, 1);
        setup.seats = vec![(Device::Keyboard, 0)];
        // The away side's line-up from another team gives way to its own.
        setup.lineups[1] = setup.lineups[0];
        let mut menus = Menus::default();
        begin_players(&mut menus, &disc, &mut setup, &story);
        let screen = &menus.players;
        assert_eq!(screen.mode, crate::choose::Mode::PickUp);
        // Home on the right with its person, its roster's tiles, nothing
        // picked; the computer's away side on the left, its three picked.
        let home = screen.side(0);
        assert_eq!(home.people, [Device::Keyboard]);
        assert!(home.picks.is_empty() && !home.done);
        assert_eq!(home.tiles.len(), disc.teams[0].roster.len().min(6));
        let away = screen.side(1);
        assert!(away.people.is_empty() && away.done);
        assert_eq!(away.lineup(), Some(setup.lineups[1]));
        assert!(
            setup.lineups[1]
                .iter()
                .all(|r| disc.teams[1].roster.contains(r))
        );
        assert!(!screen.finished(), "home has yet to pick");
        // With people on both sides, both pick at once.
        let pad = Device::Pad(Entity::from_raw_u32(9).unwrap());
        setup.seats.push((pad, 1));
        begin_players(&mut menus, &disc, &mut setup, &story);
        assert_eq!(menus.players.side(1).people, [pad]);
        assert!(!menus.players.side(1).done);
    }

    #[test]
    fn the_crew_draws_ten_locals_and_my_team_lists_the_league_a_team_a_row() {
        let Some(path) = std::env::var_os("STREET_ISO") else {
            return;
        };
        let disc = Disc::load(street_assets::iso::Iso::open(path).unwrap()).unwrap();
        let mut libc = crate::session::Libc::seeded(632);
        let locals = crate::create_team::locals(&disc, &mut libc, &|_| false);
        assert_eq!(locals.len(), 10);
        assert!(locals.iter().all(|r| (0xb8..0xea).contains(r)));
        let mut unique = locals.clone();
        unique.sort_unstable();
        unique.dedup();
        assert_eq!(unique.len(), 10);
        let story = street_sim::story::Story::default();
        let cats = team_categories(&disc, &story);
        assert_eq!(cats.len(), 4);
        assert_eq!(cats[0].rows.len(), 29);
        assert_eq!(cats[0].players.len(), 29 * 5);
        assert!(cats[2].players.is_empty(), "no legends unlocked yet");
    }

    #[test]
    fn teams_then_players_each_for_both_sides() {
        // Class 0x1b readies one side and goes live on the other; with both
        // ready, Choose Players (0x1d) does the same; then Choose Court.
        assert_eq!(after_team(0), (Page::ChooseTeam(1), 0));
        assert_eq!(after_team(1), (Page::ChoosePlayers, 0));
    }

    #[test]
    fn controllers_pick_sides() {
        let pad = |n: u32| Device::Pad(Entity::from_raw_u32(n).unwrap());
        let mut seats = vec![(Device::Keyboard, 0)];
        move_seat(&mut seats, Device::Keyboard, 1); // already Home
        assert_eq!(seats, vec![(Device::Keyboard, 0)]);
        move_seat(&mut seats, Device::Keyboard, -1); // to CPU: leaves
        assert!(seats.is_empty());
        move_seat(&mut seats, Device::Keyboard, -1); // to Away
        move_seat(&mut seats, pad(1), 1); // pad 1 to Home
        assert_eq!(seats, vec![(Device::Keyboard, 1), (pad(1), 0)]);
        for n in 2..=4 {
            move_seat(&mut seats, pad(n), 1);
        }
        assert_eq!(seats.len(), MAX_SEATS, "a fifth waits for a free seat");
        move_seat(&mut seats, Device::Keyboard, 1); // Away to CPU
        assert_eq!(seats[0], (pad(1), 0));
    }

    #[test]
    fn play_it_keeps_the_original_four_rows() {
        // The authored Play It layout (class 5) has four text rows
        // (playit.txt 2, 3, 4, 1); Watch A Game is PC Settings' row.
        assert_eq!(PLAY_IT_ROWS.len(), 4);
        assert!(!PLAY_IT_ROWS.contains(&1));
        for (row, arm) in PLAY_IT_ROWS.iter().enumerate() {
            assert_eq!(
                game_label(Page::PlayIt, row).map(|(_, k)| k),
                Some([2, 3, 4, 1][row]),
                "row {row} (arm {arm})"
            );
        }
    }

    /// Select asks on the original's screens, not on Main, the title, the
    /// rewrite's own pages, a dialog over a screen, a decided bracket, or
    /// (on User Setup and the screens after it) for a controller with no
    /// seat (`FUN_00297c40` and the rest test `+0xd8`).
    #[test]
    fn select_asks_where_the_original_asks() {
        let story = street_sim::story::Story::default();
        let asks = |stack: &[(Page, usize)], seats: &[(Device, usize)], device| {
            let menus = Menus {
                stack: stack.to_vec(),
                ..Default::default()
            };
            select_asks(menus.top().0, &menus, &story, seats, Some(device)).map(|a| a.dialog)
        };
        let main = [(Page::Title, 0), (Page::Main, 0)];
        let kb = Device::Keyboard;
        let pad = Device::Pad(Entity::from_raw_u32(3).unwrap());
        let on = |page: Page| {
            let mut s = main.to_vec();
            s.push((page, 0));
            s
        };
        assert_eq!(asks(&main, &[], kb), None);
        assert_eq!(asks(&main[..1], &[], kb), None);
        assert_eq!(asks(&on(Page::PlayIt), &[], kb), Some(0x55));
        assert_eq!(asks(&on(Page::Options), &[], kb), Some(0x85));
        assert_eq!(asks(&on(Page::Credits), &[], kb), Some(1000));
        assert_eq!(asks(&on(Page::HighScores), &[], kb), Some(0x3e9));
        assert_eq!(asks(&on(Page::Rewards), &[], kb), Some(0x44e));
        assert_eq!(asks(&on(Page::Trax), &[], kb), Some(10));
        assert_eq!(asks(&on(Page::ChooseTeam(1)), &[], kb), Some(200));
        assert_eq!(asks(&on(Page::LeagueTeam), &[], kb), Some(200));
        assert_eq!(asks(&on(Page::League), &[], kb), Some(0x457));
        assert_eq!(asks(&on(Page::Careers), &[], kb), Some(535));
        assert_eq!(asks(&on(Page::Story), &[], kb), Some(0x55));
        assert_eq!(asks(&on(Page::Bracket(0)), &[], kb), Some(0xd05));
        // The rewrite's own pages, the rolls, the dialogs over screens.
        for page in [
            Page::PcSettings,
            Page::Online,
            Page::Teams,
            Page::Controls,
            Page::CreditsRoll(crate::credits::Team::NuFx),
            Page::JoinTeam,
            Page::UpdateTeam(UpdateStage::Ask),
            Page::Completed(0),
        ] {
            assert_eq!(asks(&on(page), &[], kb), None, "{page:?}");
        }
        let mut options_ask = on(Page::Options);
        options_ask.push((Page::OptionsAsk, 1));
        assert_eq!(asks(&options_ask, &[], kb), None);
        // User Setup takes Select from a seated controller only.
        assert_eq!(asks(&on(Page::Sides), &[(pad, 0)], pad), Some(0x55));
        assert_eq!(asks(&on(Page::Sides), &[(pad, 0)], kb), None);
        assert_eq!(asks(&on(Page::ChooseCourt), &[(pad, 0)], kb), None);
        assert_eq!(asks(&on(Page::ChooseCourt), &[(pad, 0)], pad), Some(0xd06));
        // Choose Team takes it from anyone.
        assert_eq!(asks(&on(Page::ChooseTeam(0)), &[(pad, 0)], kb), Some(200));
        // Be A Legend's map, a bracket and Choose Region only from the
        // user's own pad (`+0xd4`), the one seated for the run.
        let other = Device::Pad(Entity::from_raw_u32(4).unwrap());
        for (page, id) in [
            (Page::Story, 0x55),
            (Page::Bracket(0), 0xd05),
            (Page::League, 0x457),
        ] {
            assert_eq!(asks(&on(page), &[(pad, 0)], pad), Some(id), "{page:?}");
            assert_eq!(asks(&on(page), &[(pad, 0)], other), None, "{page:?}");
            assert_eq!(asks(&on(page), &[(pad, 0)], kb), None, "{page:?}");
            assert_eq!(asks(&on(page), &[], other), Some(id), "{page:?}");
        }
    }

    /// Choose Region takes every key from the user's pad alone (the game
    /// data's `+0xd4`), the one seated for the run; with nobody seated,
    /// from anyone; the other screens from everyone.
    #[test]
    fn choose_region_takes_keys_from_the_users_pad_alone() {
        let pad = Device::Pad(Entity::from_raw_u32(3).unwrap());
        let other = Device::Pad(Entity::from_raw_u32(4).unwrap());
        let kb = Device::Keyboard;
        for page in [Page::League, Page::LeagueRegion(2)] {
            let only = listens_to(page, &[(pad, 0)]);
            assert_eq!(only, Some(pad));
            assert!(allows(only, pad));
            assert!(!allows(only, other), "{page:?}");
            assert!(!allows(only, kb), "{page:?}");
            assert_eq!(listens_to(page, &[]), None);
            assert!(allows(None, other));
        }
        for page in [
            Page::Story,
            Page::Bracket(0),
            Page::ChooseTeam(0),
            Page::Main,
        ] {
            assert_eq!(listens_to(page, &[(pad, 0)]), None, "{page:?}");
        }
    }

    /// Yes goes back to Main, the pages over it gone, putting Options back
    /// as they were and making none of the screens' leaving saves.
    #[test]
    fn yes_goes_back_to_main_without_the_screens_saves() {
        let before = street_sim::records::Settings::default();
        let mut menus = Menus {
            stack: vec![
                (Page::Title, 0),
                (Page::Main, 3),
                (Page::Options, 4),
                (Page::GameData, 2),
            ],
            data_deleted: true,
            rewards_bought: true,
            options_before: Some(before),
            ..Default::default()
        };
        let mut save = story::Save {
            path: std::env::temp_dir().join("street-exit-ask-unused.dat"),
            story: street_sim::story::Story::default(),
            last_error: None,
        };
        let mut opts = options::Options::default();
        opts.0.autosave = !before.autosave;
        opts.0.full_screen = !before.full_screen;
        let mut sound = AudioSettings::default();
        let mut difficulty = street_sim::Difficulty::Hard;
        let mut editing = ballers::Editing::default();
        let mut running = story::Running::default();
        let mut cheats = crate::cheats::Settings::default();
        exit_to_main(
            crate::exit_ask::asked(0x28).unwrap(),
            &mut menus,
            &mut save,
            (&mut opts, &mut sound, &mut difficulty),
            &mut editing,
            (&mut running, &mut cheats),
        );
        assert_eq!(menus.stack, [(Page::Title, 0), (Page::Main, 3)]);
        assert!(!menus.data_deleted && !menus.rewards_bought);
        assert!(menus.card_request.is_none(), "no card save");
        assert_eq!(opts.0.autosave, before.autosave);
        // The rewrite's own settings are not the screen's to put back.
        assert_eq!(opts.0.full_screen, !before.full_screen);
        assert_eq!(
            difficulty,
            street_sim::Difficulty::from_level(before.difficulty)
        );
        assert!(menus.options_before.is_none());
    }

    /// The rewrite's own Select uses moved to PC Settings: Online, Watch A
    /// Game and the created teams open from its rows.
    #[test]
    fn the_rewrites_own_pages_open_from_pc_settings() {
        assert_eq!(
            options::pc_items(&Default::default(), &Default::default())[options::pc::ONLINE],
            "Online"
        );
        assert_eq!(options::NOTICE.len(), 4);
        assert!(options::NOTICE[0].starts_with("Streetball Revival: an unofficial fan"));
    }

    #[test]
    fn frontend_movie_panel_placements_and_loops_match_native_screens() {
        let menus = Menus::default();
        let story = street_sim::story::Story::default();
        let class = |page: Page| screen_of(page, &menus, &story);
        // The Main Menu family (classes 3, 5, 6, 102, 103, 104, 105) runs
        // the city loop at (265, 240), 320x240: the backdrop's right window,
        // clear of the legends poster in the left frame (x < 270), and under
        // the rows. The title screen (base slot, no movie) has none.
        for page in [
            Page::Main,
            Page::PlayIt,
            Page::MyTeam,
            Page::Difficulty,
            Page::Inside,
            Page::WhatsNew,
            Page::Bobbito,
        ] {
            assert_eq!(
                loop_movie_for_page(&page, class(page)),
                Some("/MOVIES/XLP0005.NFX".into())
            );
            let rect = movie_panel_rect(&page, class(page)).unwrap();
            assert_eq!(rect, Rect::new(265.0, 240.0, 585.0, 480.0));
            assert_eq!((rect.width(), rect.height()), (320.0, 240.0));
            assert!(movie_under_rows(&page, class(page)));
        }
        assert_eq!(loop_movie_for_page(&Page::Title, class(Page::Title)), None);
        assert_eq!(movie_panel_rect(&Page::Title, class(Page::Title)), None);
        for page in [Page::Rewards, Page::MoveSlots(0), Page::Options] {
            assert!(!movie_under_rows(&page, class(page)));
        }
        assert_eq!(
            loop_movie_for_page(&Page::Options, class(Page::Options)),
            None
        );

        // The other screens' loops over their backdrops, by their slots
        // (and the runtime's loop widget on each, 2026-10-20).
        let loops = [
            (
                Page::Users,
                "/MOVIES/XLP0004.NFX",
                (464.0, 277.0, 176.0, 96.0),
            ),
            (
                Page::ChooseTeam(0),
                "/MOVIES/XLP0003.NFX",
                (0.0, 0.0, 336.0, 192.0),
            ),
            (
                Page::LeagueTeam,
                "/MOVIES/XLP0011.NFX",
                (0.0, 0.0, 336.0, 192.0),
            ),
            (
                Page::ChoosePlayers,
                "/MOVIES/XLP0002.NFX",
                (480.0, 1.0, 160.0, 416.0),
            ),
            (
                Page::ChooseCourt,
                "/MOVIES/XLP0001.NFX",
                (19.0, 0.0, 176.0, 288.0),
            ),
            (
                Page::PlayGameAsk,
                "/MOVIES/XLP0001.NFX",
                (19.0, 0.0, 176.0, 288.0),
            ),
            (
                Page::MyRules,
                "/MOVIES/XLP0010.NFX",
                (17.0, 0.0, 176.0, 288.0),
            ),
            (
                Page::Completed(0),
                "/MOVIES/XLP0006.NFX",
                (100.0, 146.0, 160.0, 208.0),
            ),
            (
                Page::Completed(1),
                "/MOVIES/XLP0016.NFX",
                (69.0, 141.0, 192.0, 208.0),
            ),
        ];
        for (page, path, (x, y, w, h)) in loops {
            assert_eq!(loop_movie_for_page(&page, class(page)), Some(path.into()));
            assert_eq!(
                movie_panel_rect(&page, class(page)),
                Some(Rect::new(x, y, x + w, y + h)),
                "{page:?}"
            );
            assert!(movie_under_rows(&page, class(page)));
        }
        // The League Challenge's Choose Players is its own class with the same
        // loop.
        assert_eq!(screen_loop(0x2b), screen_loop(0x1d));
        // No loop on Update Team (its slot's name is empty), the map
        // (0x38), Create Team, the brackets.
        for c in [0x60, 0x38, 0x35, 0x58, 0x2c, 0x07, 0x09] {
            assert_eq!(screen_loop(c), None, "{c:#x}");
        }

        // Rewards picks its movies by its own state (`moves_panel`) and
        // places them itself (`rewards_view`): the column's XLP0009 at
        // (461, 70), a trophy's XLP0012..15 at (61, 114).
        assert_eq!(
            loop_movie_for_page(&Page::Rewards, class(Page::Rewards)),
            None
        );
        assert_eq!(movie_panel_rect(&Page::Rewards, class(Page::Rewards)), None);

        // Edit Moves plays no loop (its screen's +0x15c is the editor's
        // empty one, `FUN_002d8fb0`; XLP0004 is User Setup's): the film
        // room's clip goes in the movie widget at (350, 110).
        let slots = Page::MoveSlots(0);
        assert_eq!(loop_movie_for_page(&slots, class(slots)), None);
        assert_eq!(
            movie_panel_rect(&slots, class(slots)),
            Some(Rect::new(350.0, 110.0, 574.0, 254.0))
        );

        // Move preview and film room share panel coordinates.
        assert_eq!(
            movie_panel_rect(&Page::TheMoves(0), class(Page::TheMoves(0))),
            Some(Rect::new(350.0, 110.0, 574.0, 254.0))
        );
        assert_eq!(
            movie_panel_rect(&Page::MovePick(0, 0), class(Page::MovePick(0, 0))),
            Some(Rect::new(350.0, 110.0, 574.0, 254.0))
        );
    }

    /// Screen-page availability as `FUN_00222770` sees it: rows bound to a
    /// widget are always enabled (no Screen page disables one row), and the
    /// authored nodes past the page's rows have no widget, so the graph
    /// steps over them, never onto them.
    #[test]
    fn screen_rows_bound_to_widgets_are_available_and_surplus_nodes_are_not() {
        use street_assets::font::{Element, Layout};
        let node = |id: u32, row: u32, up: u32, down: u32| Element {
            kind: 10,
            id,
            align: 0,
            props: [(25, 2), (26, up), (27, 2), (28, down), (33, 0), (34, row)]
                .into_iter()
                .collect(),
        };
        // What's New's shape: seven authored nodes in a chain.
        let chain = Layout {
            class: 103,
            elements: (0..7u32)
                .map(|k| {
                    let up = if k == 0 { u32::MAX } else { 9 + k };
                    let down = if k == 6 { u32::MAX } else { 11 + k };
                    node(10 + k, k, up, down)
                })
                .collect(),
        };
        let step = |cursor: usize, rows: usize, dir: i32| {
            look::screen_step_filtered(&chain, cursor, rows, dir, |row| {
                screen_row_available(row, rows)
            })
        };
        for rows in [2, 5, 6] {
            let mut seen = vec![0];
            let mut at = 0;
            while let Some(next) = step(at, rows, 1).filter(|&n| n != at) {
                seen.push(next);
                at = next;
            }
            assert_eq!(seen, (0..rows).collect::<Vec<_>>(), "{rows} rows");
            assert_eq!(step(rows - 1, rows, 1), Some(rows - 1));
            assert_eq!(step(rows - 1, rows, -1), Some(rows.saturating_sub(2)));
        }
        assert!(screen_row_available(4, 5));
        assert!(!screen_row_available(5, 5));

        // On the disc's Main Menu, every one of its six rows is reachable.
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let layouts = street_assets::font::layouts(&mut iso, "/FE/XLAY_FE.BIN").unwrap();
        for (class, rows) in [(3, 6), (5, 4), (6, 2), (102, 5)] {
            let layout = layouts.iter().find(|l| l.class == class).unwrap();
            let mut seen = vec![0];
            let mut at = 0;
            while let Some(next) = look::screen_step_filtered(layout, at, rows, 1, |row| {
                screen_row_available(row, rows)
            })
            .filter(|&n| n != at && !seen.contains(&n))
            {
                seen.push(next);
                at = next;
            }
            seen.sort();
            assert_eq!(seen, (0..rows).collect::<Vec<_>>(), "class {class}");
        }
    }

    /// Each screen's loop repaints its backdrop in place: laid over the
    /// doubled `XAFEBG` picture its screen shows, at the place its slot
    /// gives, a frame matches the backdrop (frame edges, rooftops, the
    /// trophies), and a few pixels away it does not. The city loop is the
    /// Main Menu backdrop's right window (the old panel over the legends
    /// poster did not match at all). Be A Legend's board is the exception:
    /// its trophy turns over a dimmed copy of the panel's picture, its own,
    /// so only its size is checked here (its place is its slot's, and the
    /// runtime's board grab puts it there within the grabs' own offset).
    #[test]
    fn screen_loops_line_up_with_their_backdrops() {
        let Ok(path) = std::env::var("STREET_ISO") else {
            return;
        };
        let mut iso = street_assets::iso::Iso::open(path).unwrap();
        let entries = street_assets::fe::read(&mut iso, "/FE/XAFEBG.BIN").unwrap();
        use crate::art::backdrop as b;
        let cases = [
            (0x03, b::MAIN, 60),
            (0x0d, crate::users::BACKDROP, 0),
            (0x1b, crate::chooseteam::BACKDROP, 0),
            (0x19, crate::chooseteam::LEAGUE_BACKDROP, 0),
            (0x1d, b::CHOOSE, 0),
            (0x21, crate::choosecourt::BACKDROP, 0),
            (0x27, crate::myrules::BACKDROP, 0),
            (0x3b, crate::completed_view::LEAGUE_BACKDROP, 0),
        ];
        assert_eq!(screen_loop(0x3c).unwrap().size, Vec2::new(192.0, 208.0));
        for (class, n, frame_no) in cases {
            let l = screen_loop(class).unwrap();
            let strips: Vec<_> = (3 * n..3 * n + 3)
                .map(|k| &entries[k].textures[0])
                .collect();
            // The backdrop's pixel under screen point (x, y), shown doubled.
            let backdrop = |x: usize, y: usize| -> [u8; 3] {
                let (mut sx, sy) = (x / 2, y / 2);
                for t in strips.iter() {
                    let w = t.width as usize;
                    if sx < w {
                        let at = (sy * w + sx) * 4;
                        return [t.rgba[at], t.rgba[at + 1], t.rgba[at + 2]];
                    }
                    sx -= w;
                }
                [0, 0, 0]
            };
            let data = iso.read(l.path).unwrap();
            let movie = street_video::nfx::Movie::parse(&data).unwrap();
            let mut decoder = street_video::mpeg2::Decoder::new(&movie.video).unwrap();
            let mut frame = Vec::new();
            for _ in 0..=frame_no {
                decoder.next_frame().unwrap().unwrap().to_rgba(&mut frame);
            }
            let (w, h) = (decoder.width, decoder.height);
            assert_eq!((w as f32, h as f32), (l.size.x, l.size.y), "{class:#x}");
            let error = |ox: usize, oy: usize| {
                let mut sum = 0u64;
                let mut count = 0u64;
                for y in (0..h).step_by(2) {
                    for x in (0..w).step_by(2) {
                        let (sx, sy) = (ox + x, oy + y);
                        if sx >= 640 || sy >= 480 {
                            continue;
                        }
                        let m = &frame[(y * w + x) * 4..][..3];
                        let b = backdrop(sx, sy);
                        sum += (0..3).map(|c| u64::from(m[c].abs_diff(b[c]))).sum::<u64>();
                        count += 3;
                    }
                }
                sum as f64 / count as f64
            };
            let (ox, oy) = (l.at.x as usize, l.at.y as usize);
            let at = error(ox, oy);
            assert!(at < 12.0, "{class:#x}: loop vs backdrop at its place: {at}");
            for (dx, dy) in [(-6, 0), (6, 0), (0, -6), (0, 6)] {
                let (x, y) = (ox as isize + dx, oy as isize + dy);
                if x < 0 || y < 0 {
                    continue;
                }
                let off = error(x as usize, y as usize);
                assert!(
                    off > at * 1.3,
                    "{class:#x} ({dx}, {dy}) fits as well: {off} vs {at}"
                );
            }
            if class == 0x03 {
                assert!(error(58, 110) > at * 3.0);
            }
        }
    }

    #[test]
    fn frontend_screen_key_dispatch_and_page_lifecycle_match_native() {
        // Native input action mappings recovered from FUN_002e8c90 in SLUS_206.51
        const ADVANCE_PRESS: u16 = 0x23; // Confirm / Cross
        const CANCEL_PRESS: u16 = 0x25; // Cancel / Triangle
        const HELP_PRESS: u16 = 0x27; // Help / Square
        const UP_PRESS: u16 = 0x1b; // Direction Up
        const DOWN_PRESS: u16 = 0x1c; // Direction Down
        const LEFT_PRESS: u16 = 0x1d; // Direction Left
        const RIGHT_PRESS: u16 = 0x1e; // Direction Right
        const START_PRESS: u16 = 0x3d; // Start Button

        assert_eq!(ADVANCE_PRESS, 35);
        assert_eq!(CANCEL_PRESS, 37);
        assert_eq!(HELP_PRESS, 39);
        assert_eq!(UP_PRESS, 27);
        assert_eq!(DOWN_PRESS, 28);
        assert_eq!(LEFT_PRESS, 29);
        assert_eq!(RIGHT_PRESS, 30);
        assert_eq!(START_PRESS, 61);

        let mut menus = Menus::default();
        assert_eq!(menus.top(), (Page::Title, 0));

        // Confirm from Title pushes Main
        menus.push(Page::Main, 0);
        assert_eq!(menus.top(), (Page::Main, 0));
        assert_eq!(menus.stack.len(), 2);

        // Cancel on Main does not pop Title; Main remains root of navigation
        if menus.stack.len() > 2 || (menus.stack.len() == 2 && menus.top().0 != Page::Main) {
            menus.stack.pop();
        }
        assert_eq!(menus.top(), (Page::Main, 0));

        // Advance into PlayIt (row 0 on Main)
        menus.push(Page::PlayIt, 0);
        assert_eq!(menus.top(), (Page::PlayIt, 0));
        assert_eq!(menus.stack.len(), 3);

        // Advance into Street School (row 4 on PlayIt)
        menus.push(Page::School, 0);
        assert_eq!(menus.top(), (Page::School, 0));
        assert_eq!(menus.stack.len(), 4);

        // Cancel unwinds back to PlayIt
        if menus.stack.len() > 2 || (menus.stack.len() == 2 && menus.top().0 != Page::Main) {
            menus.stack.pop();
        }
        assert_eq!(menus.top(), (Page::PlayIt, 0));

        // Cancel unwinds back to Main
        if menus.stack.len() > 2 || (menus.stack.len() == 2 && menus.top().0 != Page::Main) {
            menus.stack.pop();
        }
        assert_eq!(menus.top(), (Page::Main, 0));

        // Advance into Options
        menus.push(Page::Options, 0);
        assert_eq!(menus.top(), (Page::Options, 0));
        // Stepping down navigates rows
        *menus.cursor() = 3;
        assert_eq!(menus.top(), (Page::Options, 3));
        // Cancel unwinds back to Main
        if menus.stack.len() > 2 || (menus.stack.len() == 2 && menus.top().0 != Page::Main) {
            menus.stack.pop();
        }
        assert_eq!(menus.top(), (Page::Main, 0));
    }
}
