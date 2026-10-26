//! The Options page in the game's own order (`options.txt` 5..19) and what
//! each option does: Replays, Audio Options (mono), Adjust Screen (on a
//! PC, full screen), Onscreen Help, Autosave, Widescreen (Off, the PS2's
//! anamorphic On, and the rewrite's Wide), Visual Effects, Controller
//! Layout, and Trax's songs. The volumes, vibration and difficulty live
//! where they did ([`AudioSettings`], [`PadSettings`], the match rules);
//! the rest is [`Options`]. A fresh save starts from the original's
//! defaults (`Settings::default`).

use bevy::{
    camera::Viewport,
    prelude::*,
    window::{MonitorSelection, PrimaryWindow, WindowMode},
};
use street_sim::{
    Difficulty, button,
    records::{Picture, Settings},
};

use crate::{audio::AudioSettings, pads::PadSettings};

/// The options that are not volumes, vibration or difficulty.
#[derive(Resource, Default, Clone, Copy)]
pub struct Options(pub Settings);

/// `--widescreen off|on|wide`: the picture for this run, over the saved
/// option (for checking the three pictures; nothing is saved).
#[derive(Resource, Clone, Copy)]
pub struct PictureOverride(pub Picture);

/// The picture to draw: the run's override, else the saved option's.
pub fn picture(o: Option<&Options>, over: Option<&PictureOverride>) -> Picture {
    over.map_or_else(
        || o.map_or(Picture::Standard, |o| o.0.picture()),
        |over| over.0,
    )
}

/// The Widescreen row's choices in order: Off, On, Wide.
pub const PICTURES: [(Picture, &str); 3] = [
    (Picture::Standard, "Off"),
    (Picture::Anamorphic, "On"),
    (Picture::Wide, "Wide"),
];

pub fn picture_label(picture: Picture) -> &'static str {
    PICTURES
        .iter()
        .find(|&&(value, _)| value == picture)
        .map_or("Off", |&(_, label)| label)
}

pub fn step_picture(picture: Picture, step: i32) -> Picture {
    let ordinal = PICTURES
        .iter()
        .position(|&(value, _)| value == picture)
        .unwrap_or(0);
    PICTURES[(ordinal as i32 + step).rem_euclid(PICTURES.len() as i32) as usize].0
}

/// Pause's authored choices at 0x48e210 skip text 0x5813 (Behind).
/// FUN_002bdc60 saves ordinal 0..4; FUN_003038b0 maps the last to mode 5.
pub const CAMERA_TYPES: [(u8, &str); 5] = [
    (0, "Street"),
    (1, "Court"),
    (2, "Slide"),
    (3, "Solid"),
    (5, "Wide"),
];

pub fn camera_label(mode: u8) -> &'static str {
    CAMERA_TYPES
        .iter()
        .find(|&&(value, _)| value == mode)
        .map_or("Street", |&(_, label)| label)
}

pub fn step_camera(mode: u8, step: i32) -> u8 {
    let ordinal = CAMERA_TYPES
        .iter()
        .position(|&(value, _)| value == mode)
        .unwrap_or(0);
    CAMERA_TYPES[(ordinal as i32 + step).rem_euclid(CAMERA_TYPES.len() as i32) as usize].0
}

/// The Options screen's rows (`FUN_002a4c58`: class 0x28, layout 40,
/// backdrop 0x1c), in the order list 55 shows them (`options.png`,
/// `options_scrolled*.png`), each a label (kind 19, ids 1, 16, 126, 15, 7,
/// 5, 6, 4, 3, 10, 14, 12, 13, 9, 17) and its value widget.
pub mod row {
    pub const DIFFICULTY: usize = 0;
    pub const LAYOUT: usize = 1;
    pub const GAME_DATA: usize = 2;
    pub const TRAX: usize = 3;
    pub const AUDIO: usize = 4;
    pub const SPEECH: usize = 5;
    pub const MUSIC: usize = 6;
    pub const SFX: usize = 7;
    pub const REPLAYS: usize = 8;
    pub const HELP: usize = 9;
    pub const EFFECTS: usize = 10;
    pub const AUTOSAVE: usize = 11;
    pub const WIDESCREEN: usize = 12;
    pub const SCREEN: usize = 13;
    pub const EXIT: usize = 14;
}

/// How many rows the screen has.
pub const ROWS: usize = row::EXIT + 1;

/// Each row's label (`options.txt`).
const LABELS: [usize; ROWS] = [6, 18, 17, 5, 11, 9, 10, 8, 7, 13, 16, 14, 15, 12, 19];
/// Each row's help line (`options.txt`, `FUN_002a5150` by the focused row's
/// element).
const HELPS: [usize; ROWS] = [30, 43, 42, 29, 36, 34, 35, 33, 32, 38, 41, 39, 40, 37, 44];

/// A row's help line in `options.txt`.
pub fn help(row: usize) -> Option<usize> {
    HELPS.get(row).copied()
}

/// A row's label in `options.txt`.
pub fn label(row: usize) -> Option<usize> {
    LABELS.get(row).copied()
}

const DIFFICULTIES: [&str; 3] = ["Got Game (Basic)", "Mad Game (Medium)", "Legendary (Hard)"];
const REPLAYS: [&str; 3] = ["Always", "Automatic", "Manual"];
const AUDIO: [&str; 3] = ["Mono", "Stereo", "Dolby Pro Logic II"];

fn on(b: bool) -> &'static str {
    if b { "On" } else { "Off" }
}

fn level(d: Difficulty) -> usize {
    usize::from(d.level())
}

/// What a row's value widget shows: words (`options.txt` by index, and as
/// written without the disc), a volume's slider (0..10), or nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Value {
    Words(usize, &'static str),
    Slider(u8),
}

/// A row's value (the option widgets `0x35` difficulty, `0x33` audio,
/// `0x36` replays, `0x31` help, `0x2e` effects, `0x30` autosave and the
/// widescreen's; the sliders `0x6f..0x71`).
pub fn value(at: usize, o: &Settings, sound: &AudioSettings, d: Difficulty) -> Option<Value> {
    let flag = |b: bool| Value::Words(if b { 27 } else { 28 }, on(b));
    Some(match at {
        row::DIFFICULTY => Value::Words(21 + level(d), DIFFICULTIES[level(d)]),
        row::AUDIO => {
            let a = usize::from(o.audio.min(2));
            Value::Words(24 + a, AUDIO[a])
        }
        row::SPEECH => Value::Slider(sound.speech),
        row::MUSIC => Value::Slider(sound.music),
        row::SFX => Value::Slider(sound.effects),
        row::REPLAYS => {
            let r = usize::from(o.replays.min(2));
            Value::Words(47 + r, REPLAYS[r])
        }
        row::HELP => flag(o.help),
        row::EFFECTS => flag(o.visual_effects),
        row::AUTOSAVE => flag(o.autosave),
        row::WIDESCREEN => flag(o.widescreen),
        _ => return None,
    })
}

/// The page's rows, "Label  < value >" where there is a value (the plain
/// list without the disc's art).
pub fn items(o: &Settings, sound: &AudioSettings, d: Difficulty) -> Vec<String> {
    const PLAIN: [&str; ROWS] = [
        "Difficulty",
        "Controller Layout",
        "Game Data",
        "Trax",
        "Audio Options",
        "Speech Volume",
        "Music Volume",
        "SFX Volume",
        "Replays",
        "Onscreen Help",
        "Visual Effects",
        "Autosave",
        "Widescreen",
        "Adjust Screen",
        "Exit",
    ];
    (0..ROWS)
        .map(|at| match value(at, o, sound, d) {
            Some(Value::Words(_, words)) => format!("{}  < {words} >", PLAIN[at]),
            Some(Value::Slider(n)) => format!("{}  < {n} >", PLAIN[at]),
            None => PLAIN[at].to_owned(),
        })
        .collect()
}

/// The volume rows and their levels, for the page's sliders.
pub fn volumes(sound: &AudioSettings) -> [(usize, u8); 3] {
    [
        (row::SPEECH, sound.speech),
        (row::MUSIC, sound.music),
        (row::SFX, sound.effects),
    ]
}

/// Left or right (`step` -1 or +1) on a row: the option widgets go round
/// their choices, the sliders stop at 0 and 10.
pub fn step(
    at: usize,
    step: i32,
    o: &mut Settings,
    sound: &mut AudioSettings,
    difficulty: &mut Difficulty,
) {
    let cycle = |v: u8, n: i32| (i32::from(v) + step).rem_euclid(n) as u8;
    let volume = |v: &mut u8| *v = (i32::from(*v) + step).clamp(0, 10) as u8;
    match at {
        row::DIFFICULTY => *difficulty = Difficulty::from_level(cycle(difficulty.level(), 3)),
        row::REPLAYS => o.replays = cycle(o.replays, 3),
        row::SFX => volume(&mut sound.effects),
        row::SPEECH => volume(&mut sound.speech),
        row::MUSIC => volume(&mut sound.music),
        row::AUDIO => o.audio = cycle(o.audio, 3),
        row::HELP => o.help = !o.help,
        row::AUTOSAVE => o.autosave = !o.autosave,
        row::WIDESCREEN => o.widescreen = !o.widescreen,
        row::EFFECTS => o.visual_effects = !o.visual_effects,
        _ => {}
    }
}

/// Everything to keep, from the live settings.
/// Songs the save switched on by itself ([`street_sim::story::Story::court_granted`])
/// reach the live options and the playlist.
pub fn follow_switched_songs(
    mut save: ResMut<crate::story::Save>,
    mut opts: ResMut<Options>,
    mut sounds: ResMut<crate::audio::Sounds>,
) {
    let on = save.story.songs_switched_on;
    if on == 0 {
        return;
    }
    save.story.songs_switched_on = 0;
    opts.0.songs_off &= !on;
    sounds.trax(opts.0.songs_off, opts.0.sequential, opts.0.interactive);
}

pub fn gather(o: &Settings, sound: &AudioSettings, pads: &PadSettings, d: Difficulty) -> Settings {
    Settings {
        music: sound.music,
        effects: sound.effects,
        speech: sound.speech,
        vibration: pads.vibration,
        difficulty: d.level(),
        ..*o
    }
}

/// Whether the screen's options differ from `before` (`FUN_002a53b0`
/// compares the volumes, difficulty, replays, audio, help, effects and
/// autosave against the settings): Triangle then asks before it cancels.
pub fn changed(before: &Settings, now: &Settings) -> bool {
    (
        before.music,
        before.effects,
        before.speech,
        before.difficulty,
        before.replays,
        before.audio,
        before.help,
        before.visual_effects,
        before.autosave,
        before.widescreen,
    ) != (
        now.music,
        now.effects,
        now.speech,
        now.difficulty,
        now.replays,
        now.audio,
        now.help,
        now.visual_effects,
        now.autosave,
        now.widescreen,
    )
}

// ---------- the rewrite's own settings ----------

/// The rewrite's settings, which the original has no rows for: kept off
/// the Options screen on a page of their own (`Page::PcSettings`), which
/// Adjust Screen opens (the original's repositions a TV picture, which a
/// window does not need). The rewrite's own pages open from it too, off
/// the original's screens (whose Select asks "Exit to Main Menu?",
/// [`crate::exit_ask`]): Online, Watch A Game and the created teams' list.
pub mod pc {
    /// A window or full screen.
    pub const DISPLAY: usize = 0;
    /// The rewrite's Wide picture (with Widescreen On).
    pub const WIDE: usize = 1;
    /// The controllers' rumble.
    pub const VIBRATION: usize = 2;
    /// Host or join a match over the network (`Page::Online`).
    pub const ONLINE: usize = 3;
    /// The computer on both sides.
    pub const WATCH: usize = 4;
    /// The created teams, to edit or delete one (`Page::Teams`).
    pub const TEAMS: usize = 5;
    pub const BACK: usize = 6;
}

/// The project's own notice, under PC Settings' rows: what this game is
/// and whose the original is (the rewrite's credits page before it built
/// the original's credits screen).
pub const NOTICE: [&str; 6] = [
    "Streetball Revival: an unofficial fan",
    "project, written from scratch.",
    "Every model, texture, animation and",
    "number is read from your own disc.",
    "",
    "Not affiliated with the original's makers.",
];

/// The rewrite's settings page's rows.
pub fn pc_items(o: &Settings, pads: &PadSettings) -> Vec<String> {
    vec![
        format!(
            "Display  < {} >",
            if o.full_screen {
                "Full Screen"
            } else {
                "Window"
            }
        ),
        format!("Wide Picture  < {} >", on(o.wide)),
        format!("Vibration  < {} >", on(pads.vibration)),
        "Online".into(),
        "Watch A Game".into(),
        "Created Teams".into(),
        "Back".into(),
    ]
}

/// The rewrite's settings page's help lines.
pub fn pc_help(at: usize) -> &'static str {
    match at {
        pc::DISPLAY => "Play in a window or full screen.",
        pc::WIDE => "With Widescreen on, show the whole 4:3 picture and more at the sides.",
        pc::VIBRATION => "Turn controller vibration on or off.",
        pc::ONLINE => "Host or join a match over the network.",
        pc::WATCH => "Watch the computer play both sides.",
        pc::TEAMS => "Edit or delete a created team.",
        _ => "Return to Options.",
    }
}

/// Whether the rewrite's own settings changed (its PC Settings page, in
/// Adjust Screen's place): leaving Options saves them as it saves the
/// original's.
pub fn pc_changed(before: &Settings, now: &Settings) -> bool {
    (before.full_screen, before.wide, before.vibration)
        != (now.full_screen, now.wide, now.vibration)
}

/// Left, right or a press on the rewrite's settings page.
pub fn pc_step(at: usize, o: &mut Settings, pads: &mut PadSettings) {
    match at {
        pc::DISPLAY => o.full_screen = !o.full_screen,
        pc::WIDE => o.wide = !o.wide,
        pc::VIBRATION => pads.vibration = !pads.vibration,
        _ => {}
    }
}

// ---------- controller layouts ----------// ---------- controller layouts ----------

/// The layouts' names (`contconfig.txt` 9..11).
pub const LAYOUT_NAMES: [&str; 3] = ["Layout A", "Layout B", "Layout C"];

/// The Controller Layout page: each player's layout, then Back.
pub fn layout_items(o: &Settings) -> Vec<String> {
    let mut out: Vec<String> = o
        .layouts
        .iter()
        .enumerate()
        .map(|(k, &l)| {
            format!(
                "Player {}  < {} >",
                k + 1,
                LAYOUT_NAMES[usize::from(l.min(2))]
            )
        })
        .collect();
    out.push("Back".into());
    out
}

/// What a layout puts on the four face buttons, for the page's help line:
/// Circle/B, Cross/A, Square/X, Triangle/Y in the style's own names.
pub fn layout_help(layout: u8, style: crate::pads::ButtonStyle) -> String {
    use crate::pads::ButtonStyle as S;
    let [circle, cross, square, triangle] = match style {
        S::Xbox => ["B", "A", "X", "Y"],
        S::Keyboard => ["J", "K", "L", "I"],
        S::PlayStation => ["Circle", "Cross", "Square", "Triangle"],
    };
    let [c, x, s, t] = match layout {
        1 => [
            "pass",
            "shoot, block / rebound",
            "block / rebound",
            "trick move / steal",
        ],
        2 => [
            "shoot, block / rebound",
            "pass",
            "trick move / steal",
            "block / rebound",
        ],
        _ => ["shoot", "pass", "trick move / steal", "block / rebound"],
    };
    format!("{circle} {c}   {cross} {x}   {square} {s}   {triangle} {t}")
}

/// A controller's face buttons through its layout (`USRGAME1..3.CFG`).
/// `held` names the buttons as layout A does (Circle shoot, Cross pass,
/// Square trick, Triangle block). Where a layout puts shoot and block on
/// one button, it shoots while the team has the ball and blocks or
/// rebounds otherwise, as the game's two bindings on it do.
pub fn remap(layout: u8, held: u16, offence: bool) -> u16 {
    const FACE: u16 = button::SHOOT | button::PASS | button::TRICK | button::JUMP;
    let has = |b: u16| held & b != 0;
    let shoot_or_block = if offence { button::SHOOT } else { button::JUMP };
    let mut face = 0;
    let mut put = |on: bool, b: u16| {
        if on {
            face |= b;
        }
    };
    let (circle, cross, square, triangle) = (
        has(button::SHOOT),
        has(button::PASS),
        has(button::TRICK),
        has(button::JUMP),
    );
    match layout {
        1 => {
            put(circle, button::PASS);
            put(cross, shoot_or_block);
            put(square, button::JUMP);
            put(triangle, button::TRICK);
        }
        2 => {
            put(circle, shoot_or_block);
            put(cross, button::PASS);
            put(square, button::TRICK);
            put(triangle, button::JUMP);
        }
        _ => return held,
    }
    (held & !FACE) | face
}

// ---------- the systems ----------

/// Visual Effects is on (a run condition for the effects' systems).
pub fn effects_on(o: Option<Res<Options>>) -> bool {
    o.is_none_or(|o| o.0.visual_effects)
}

pub struct OptionsPlugin;

impl Plugin for OptionsPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Options>()
            .add_systems(Startup, spawn_bars)
            .add_systems(Update, (apply, letterbox));
    }
}

/// Behind every camera: black, for the bars around a 4:3 picture. It runs
/// only while there are bars (`letterbox`): the window keeps the first
/// camera's clear, so with it running the pictures across the whole window
/// would lose the sky's clear colour to its black.
fn spawn_bars(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        Camera {
            order: -100,
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            is_active: false,
            ..default()
        },
    ));
}

/// Full screen or a window, and mono, as the options change.
fn apply(
    o: Res<Options>,
    mut windows: Query<&mut Window, With<PrimaryWindow>>,
    mut sounds: ResMut<crate::audio::Sounds>,
) {
    if !o.is_changed() {
        return;
    }
    crate::audio::MONO.store(o.0.audio == 0, std::sync::atomic::Ordering::Relaxed);
    sounds.trax(o.0.songs_off, o.0.sequential, o.0.interactive);
    if let Ok(mut w) = windows.single_mut() {
        let mode = if o.0.full_screen {
            WindowMode::BorderlessFullscreen(MonitorSelection::Current)
        } else {
            WindowMode::Windowed
        };
        if w.mode != mode {
            w.mode = mode;
        }
    }
}

/// The court's area in a window `w` by `h` (physical pixels), as `(x,
/// width)`, when it does not fill the window: the middle 4:3 with
/// Widescreen off.
pub fn scene_box(w: u32, h: u32, picture: Picture) -> Option<(u32, u32)> {
    (picture == Picture::Standard && w * 3 > h * 4).then(|| {
        let width = h * 4 / 3;
        ((w - width) / 2, width)
    })
}

/// The 2D layer's area (the menus, the HUD, the loading pictures): the
/// court's 4:3 with Widescreen off; On and Wide, the window up to 16:9 and
/// the middle 16:9 of a wider one, so an ultrawide window widens the court
/// but keeps the 2D art at its PS2 widescreen shape instead of stretching
/// it.
pub fn ui_box(w: u32, h: u32, picture: Picture) -> Option<(u32, u32)> {
    if picture == Picture::Standard {
        return scene_box(w, h, picture);
    }
    (w * 9 > h * 16).then(|| {
        let width = h * 16 / 9;
        ((w - width) / 2, width)
    })
}

/// The 2D layer's left edge in the window's logical pixels (where the
/// court's camera puts a point, less this, is where the HUD draws it).
pub fn ui_left(window: &Window, picture: Picture) -> f32 {
    let (w, h) = (window.physical_width(), window.physical_height());
    let ui = ui_box(w, h, picture).map_or(0, |(x, _)| x);
    let scene = scene_box(w, h, picture).map_or(0, |(x, _)| x);
    (ui as f32 - scene as f32) / window.scale_factor()
}

/// Widescreen off: every picture in the middle 4:3 of the window. On and
/// Wide: the court across the whole window, the 2D layer across it up to
/// 16:9 ([`ui_box`]).
fn letterbox(
    o: Res<Options>,
    over: Option<Res<PictureOverride>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<(
        &mut Camera,
        Option<&bevy::camera::RenderTarget>,
        Has<Camera3d>,
    )>,
) {
    let Ok(window) = windows.single() else {
        return;
    };
    let (w, h) = (window.physical_width(), window.physical_height());
    let shown = picture(Some(&o), over.as_deref());
    let viewport = |b: Option<(u32, u32)>| {
        b.map(|(x, width)| Viewport {
            physical_position: UVec2::new(x, 0),
            physical_size: UVec2::new(width, h),
            ..default()
        })
    };
    let scene = viewport(scene_box(w, h, shown));
    let ui = viewport(ui_box(w, h, shown));
    let to_window = |target: Option<&bevy::camera::RenderTarget>| {
        !matches!(target, Some(bevy::camera::RenderTarget::Image(_)))
    };
    // The bars' black runs while some picture leaves the window's edges
    // bare: the court's 4:3, or the 2D layer's box with no court behind it
    // (a court across the window keeps its own clear colour).
    let court = cameras
        .iter()
        .any(|(c, target, is_3d)| is_3d && c.is_active && to_window(target));
    let bars = scene.is_some() || (ui.is_some() && !court);
    for (mut camera, target, is_3d) in &mut cameras {
        // The pictures drawn into an image (the 3D baller's panel, the
        // star board's) keep their own size.
        if !to_window(target) {
            continue;
        }
        if camera.order == -100 {
            if camera.is_active != bars {
                camera.is_active = bars;
            }
            continue;
        }
        let want = if is_3d { &scene } else { &ui };
        let same = match (&camera.viewport, want) {
            (None, None) => true,
            (Some(a), Some(b)) => {
                a.physical_position == b.physical_position && a.physical_size == b.physical_size
            }
            _ => false,
        };
        if !same {
            camera.viewport = want.clone();
        }
    }
}

#[cfg(test)]
mod picture_tests {
    use super::*;

    #[test]
    fn an_ultrawide_window_widens_the_court_and_boxes_the_2d_at_16_9() {
        // 21:9: the court fills it; the 2D layer keeps 16:9 in the middle.
        assert_eq!(scene_box(2520, 1080, Picture::Wide), None);
        assert_eq!(ui_box(2520, 1080, Picture::Wide), Some((300, 1920)));
        assert_eq!(ui_box(5120, 1440, Picture::Anamorphic), Some((1280, 2560)));
        // 16:9 and narrower: everything across the window.
        assert_eq!(ui_box(1920, 1080, Picture::Wide), None);
        assert_eq!(ui_box(1600, 1200, Picture::Anamorphic), None);
        // Widescreen off: both in the middle 4:3.
        assert_eq!(scene_box(1920, 1080, Picture::Standard), Some((240, 1440)));
        assert_eq!(ui_box(1920, 1080, Picture::Standard), Some((240, 1440)));
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn pause_camera_choices_skip_behind_and_wrap_both_directions() {
        let mut mode = 0;
        let modes: Vec<_> = (0..5)
            .map(|_| {
                let current = mode;
                mode = super::step_camera(mode, 1);
                current
            })
            .collect();
        assert_eq!(modes, [0, 1, 2, 3, 5]);
        assert_eq!(mode, 0);
        assert_eq!(super::step_camera(0, -1), 5);
        assert_eq!(super::camera_label(5), "Wide");
    }

    use super::*;

    #[test]
    fn layouts_move_the_face_buttons() {
        let circle = button::SHOOT;
        let cross = button::PASS;
        // Layout A is the pad as it is.
        assert_eq!(remap(0, circle | button::R1, true), circle | button::R1);
        // Layout B: Circle passes, Cross shoots with the ball and blocks
        // without, Triangle jukes.
        assert_eq!(remap(1, circle, true), button::PASS);
        assert_eq!(remap(1, cross, true), button::SHOOT);
        assert_eq!(remap(1, cross, false), button::JUMP);
        assert_eq!(remap(1, button::JUMP, true), button::TRICK);
        // Layout C: Circle shoots or blocks; the turbos stay.
        assert_eq!(
            remap(2, circle | button::L2, false),
            button::JUMP | button::L2
        );
        assert_eq!(remap(2, cross, false), button::PASS);
    }

    #[test]
    fn every_row_steps_and_wraps() {
        let mut o = Settings::default();
        let mut sound = AudioSettings::default();
        let mut pads = PadSettings::default();
        let mut d = Difficulty::Medium;
        let before = gather(&o, &sound, &pads, d);
        step(row::REPLAYS, 1, &mut o, &mut sound, &mut d);
        step(row::REPLAYS, 1, &mut o, &mut sound, &mut d);
        assert_eq!(o.replays, 0); // Manual wraps to Always
        step(row::DIFFICULTY, -1, &mut o, &mut sound, &mut d);
        assert_eq!(d, Difficulty::Easy);
        step(row::MUSIC, 9, &mut o, &mut sound, &mut d);
        assert_eq!(sound.music, 10);
        step(row::AUTOSAVE, 1, &mut o, &mut sound, &mut d);
        assert!(!o.autosave);
        let kept = gather(&o, &sound, &pads, d);
        assert_eq!((kept.music, kept.difficulty, kept.autosave), (10, 0, false));
        assert!(changed(&before, &kept));
        assert!(!changed(&kept, &kept));
        assert_eq!(items(&o, &sound, d).len(), ROWS);
        // The rewrite's own rows are on their own page, the rows that open
        // its own pages doing nothing on left and right.
        pc_step(pc::VIBRATION, &mut o, &mut pads);
        assert!(!pads.vibration);
        let items = pc_items(&o, &pads);
        assert_eq!(items.len(), pc::BACK + 1);
        assert_eq!(
            [&items[pc::ONLINE], &items[pc::WATCH], &items[pc::TEAMS]],
            ["Online", "Watch A Game", "Created Teams"]
        );
        let kept = o;
        for row in [pc::ONLINE, pc::WATCH, pc::TEAMS, pc::BACK] {
            pc_step(row, &mut o, &mut pads);
        }
        assert_eq!((o, pads.vibration), (kept, false));
    }

    /// The original's rows in the screen's order, each with its label and
    /// help (`options.png`: Difficulty, Controller Layout, Game Data, the publisher
    /// Trax, Audio Options, the three volumes; then Replays .. Exit).
    #[test]
    fn the_rows_follow_the_screen() {
        let o = Settings::default();
        let sound = AudioSettings::default();
        let labels: Vec<usize> = (0..ROWS).map(|k| label(k).unwrap()).collect();
        assert_eq!(
            labels,
            [6, 18, 17, 5, 11, 9, 10, 8, 7, 13, 16, 14, 15, 12, 19]
        );
        assert_eq!(help(row::DIFFICULTY), Some(30));
        assert_eq!(help(row::EXIT), Some(44));
        assert_eq!(
            value(row::DIFFICULTY, &o, &sound, Difficulty::Easy),
            Some(Value::Words(21, "Got Game (Basic)"))
        );
        assert_eq!(
            value(row::AUDIO, &o, &sound, Difficulty::Easy),
            Some(Value::Words(25, "Stereo"))
        );
        assert_eq!(
            value(row::REPLAYS, &o, &sound, Difficulty::Easy),
            Some(Value::Words(48, "Automatic"))
        );
        assert_eq!(
            value(row::SPEECH, &o, &sound, Difficulty::Easy),
            Some(Value::Slider(10))
        );
        assert_eq!(value(row::LAYOUT, &o, &sound, Difficulty::Easy), None);
    }

    /// Widescreen is Off and On (the PS2's anamorphic picture); the
    /// rewrite's Wide is its own page's row, over Widescreen On; a fresh
    /// save starts Off, as the original does.
    #[test]
    fn widescreen_cycles_the_three_pictures() {
        let mut o = Settings::default();
        let mut sound = AudioSettings::default();
        let mut pads = PadSettings::default();
        let mut d = Difficulty::Easy;
        assert_eq!(o.picture(), Picture::Standard);
        step(row::WIDESCREEN, 1, &mut o, &mut sound, &mut d);
        assert_eq!(o.picture(), Picture::Anamorphic);
        pc_step(pc::WIDE, &mut o, &mut pads);
        assert_eq!(o.picture(), Picture::Wide);
        step(row::WIDESCREEN, -1, &mut o, &mut sound, &mut d);
        assert_eq!(o.picture(), Picture::Standard);
        step(row::WIDESCREEN, 1, &mut o, &mut sound, &mut d);
        assert_eq!(o.picture(), Picture::Wide);
        assert!(items(&o, &sound, d)[row::WIDESCREEN].contains("On"));
        assert_eq!(picture(None, None), Picture::Standard);
        assert_eq!(
            picture(
                Some(&Options(o)),
                Some(&PictureOverride(Picture::Anamorphic))
            ),
            Picture::Anamorphic
        );
        assert_eq!(picture(Some(&Options(o)), None), Picture::Wide);
    }
}
