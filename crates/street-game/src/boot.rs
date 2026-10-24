//! The boot's own loading screen (`FUN_002f57f8`) and the memory card's
//! check after it (`FUN_0029dc18`), before the boot's movies (the first
//! title's transition, [`crate::transition`]).
//!
//! While the boot loads, the game's own frame is held back (`DAT_00403af4`
//! set, `FUN_003129c8` drawing nothing) and a callback run as the loads go
//! (`FUN_002f4d00`) draws `xldscrn.bin` (`xldfrscn.bin` on a console set to
//! French, `FUN_00342088`), a 320x240 picture over the PS2's whole 640x448
//! frame (`FUN_002f57f8`: 640 by 448 about the frame's middle), and over it
//! the bar: a gold widget (`FUN_002332d0`, 0xfffdb813) 7 high at (226, 380)
//! of the frame, (226, 396) in the layouts' space, whose width grows 0.14 a
//! call (`DAT_004005d0`) up to 0.95 of 146 (`DAT_004005cc`) and is the whole
//! 146 once the front end's packages are in (`DAT_004005d4`, set by
//! `FUN_0029dca8` between its last two loads). `FUN_002f4f80` ends it, and
//! the boot's screen (class 4) makes the card widget and loads every
//! section (`+0x174` with 0x7f): Checking, then the card's state.
//!
//! The rewrite has nothing to load there, so it shows the picture and the
//! bar as the original's boot shows them, measured on three boots of the
//! project's PCSX2 runtime (PINE on 0x403af4, the bar's width at 0x4d81c0
//! and 0x4005d4 against the 100 Hz timer 0x4b32e4, PrintWindow frames each
//! 0.1 s): the picture shows 0.47 s after the flag is set, the bar stays
//! empty for 2.44 s, grows to 1.12 in two calls, stands while the IOP's
//! modules load (`FUN_002f5578`; its own 35 more is lost, the bar widget
//! being made anew each call), grows at 8.17 a second (0.14 a call, 58
//! calls a second) to 69.58, and is whole 12.57 s in; the picture stands
//! until the card's first dialog, 13.62 s in (13.53 to 13.68). The flag is
//! set at 103 or 104 hundredths and cleared at 1484 to 1492 on every boot.
//! No press is taken: the boot takes none while it loads.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};

use crate::{
    Screen,
    card::{self, Holds},
    look::{self, Piece},
    menu::MenuInput,
    setup::Disc,
};

/// The picture's file (the US disc's; `xldfrscn.bin` for French).
pub const PICTURE: &str = "/xldscrn.bin";
/// The picture's part of its texture: 320x240 from the top left.
const SHOWN: Vec2 = Vec2::new(320.0, 240.0);
/// Where it stands: the PS2's whole frame, the layouts' rows 16..464.
pub const PICTURE_AT: Rect = Rect {
    min: Vec2::new(0.0, 16.0),
    max: Vec2::new(640.0, 464.0),
};
/// The bar's left and top in the layouts' space, its height and colour.
pub const BAR_AT: Vec2 = Vec2::new(226.0, 396.0);
pub const BAR_HEIGHT: f32 = 7.0;
pub const BAR_COLOUR: [u8; 3] = [0xfd, 0xb8, 0x13];
/// The bar's whole width (`DAT_004005cc`).
pub const BAR_WIDTH: f32 = 146.0;
/// How long the picture stands before the card's check (seconds).
pub const PICTURE_SECONDS: f64 = 13.62;
/// The boot's screen (`FUN_0029dc18`'s, the factory's class 4).
pub const BOOT_CLASS: u16 = 0x04;

/// The bar's measured width at hundredths of a second since the picture
/// showed; it holds the last.
const FILL: [(f64, f32); 6] = [
    (0.0, 0.0),
    (244.0, 0.0),
    (251.0, 1.12),
    (416.0, 1.12),
    (1254.0, 69.58),
    (1257.0, BAR_WIDTH),
];

/// The bar's width `t` seconds after the picture showed, as the original's
/// boot fills it.
pub fn bar_width(t: f64) -> f32 {
    let h = t * 100.0;
    let mut last = FILL[0];
    for &(at, w) in &FILL {
        if h < at {
            let (a0, w0) = last;
            if at <= a0 {
                return w;
            }
            let k = ((h - a0) / (at - a0)) as f32;
            // The jump to the whole width is one call, not a slope.
            if w == BAR_WIDTH {
                return w0;
            }
            return w0 + (w - w0) * k;
        }
        last = (at, w);
    }
    last.1
}

/// The boot screen's state.
#[derive(Resource, Default)]
pub struct Boot {
    picture: Option<Handle<Image>>,
    since: f64,
    /// The card's check has begun.
    checked: bool,
    /// The file's state as the boot found it (it was read at the start).
    holds: Option<Holds>,
}

pub struct BootPlugin;

impl Plugin for BootPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Boot>()
            .add_systems(OnEnter(Screen::Boot), start)
            .add_systems(Update, (run, draw).chain().run_if(in_state(Screen::Boot)));
    }
}

/// The picture, read from the disc (or the HD pack's for it).
fn picture(disc: &mut Disc, images: &mut Assets<Image>) -> Option<Handle<Image>> {
    let sheet = street_assets::fe::sheet(&mut disc.iso, PICTURE).ok()?;
    let t = sheet.textures.first()?;
    let t = street_render::hires::swap(t);
    // The part that shows, at the pack's scale.
    let scale = t.width as f32 / 512.0;
    let (w, h) = ((SHOWN.x * scale) as u32, (SHOWN.y * scale) as u32);
    let mut rgba = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h.min(t.height) {
        let row = (y * t.width * 4) as usize;
        let line = &t.rgba[row..row + (w.min(t.width) * 4) as usize];
        for px in line.as_chunks::<4>().0 {
            // Drawn opaque over the cleared frame.
            rgba.extend_from_slice(&[px[0], px[1], px[2], 0xff]);
        }
    }
    Some(images.add(Image::new(
        Extent3d {
            width: w.min(t.width),
            height: h.min(t.height),
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )))
}

fn start(
    mut commands: Commands,
    mut boot: ResMut<Boot>,
    mut disc: ResMut<Disc>,
    mut images: ResMut<Assets<Image>>,
    time: Res<Time<Real>>,
    save: Res<crate::story::Save>,
) {
    // The boot's frame is black under its picture and the card's dialogs.
    commands.spawn((
        Camera2d,
        Camera {
            clear_color: ClearColorConfig::Custom(Color::BLACK),
            ..default()
        },
        DespawnOnExit(Screen::Boot),
    ));
    *boot = Boot {
        picture: picture(&mut disc, &mut images),
        since: time.elapsed_secs_f64(),
        checked: false,
        holds: Some(Holds::of(&save.path)),
    };
}

/// The picture's time, then the card's check (`FUN_0029dc18`: Checking,
/// then Loading and "Load successful." for a file, "No game
/// data found" without one), then the title through its first transition,
/// the boot's movies ([`crate::transition::BOOT_REEL`]): the boot's screen
/// (class 4) asks for the title (class 0x63) as the card's last message
/// goes, and the menus open under the movies.
#[allow(clippy::too_many_arguments)]
fn run(
    time: Res<Time<Real>>,
    mut boot: ResMut<Boot>,
    mut card: ResMut<card::Card>,
    input: MenuInput,
    mut save: ResMut<crate::story::Save>,
    opts: Res<crate::options::Options>,
    mut next: ResMut<NextState<Screen>>,
    mut transition: ResMut<crate::transition::Transition>,
) {
    if time.elapsed_secs_f64() - boot.since < crate::pace::hold(PICTURE_SECONDS, crate::pace::BOOT)
    {
        return;
    }
    let holds = boot.holds.unwrap_or(Holds::Nothing);
    if !boot.checked {
        boot.checked = true;
        boot.picture = None;
        card.flow = Some(card::Flow::new(card::Op::Boot, holds, opts.0.autosave));
    }
    let Some(flow) = card.flow.as_mut() else {
        return;
    };
    card::keys(flow, &input, f64::from(time.delta_secs()));
    match flow.work() {
        // The file was read as the game started; the card says how.
        Some(card::Work::Read) => flow.read(holds == Holds::Data),
        Some(card::Work::Reset) => {
            save.story = Default::default();
            save.story.trophy_rewards();
        }
        Some(card::Work::Write) => card::write(flow, &mut save),
        None => {}
    }
    if flow.done {
        card.flow = None;
        transition.begin(
            crate::transition::Change::new(
                BOOT_CLASS,
                crate::attract::TITLE,
                crate::transition::Kind::Forward,
                false,
                None,
            ),
            time.elapsed_secs_f64(),
        );
        next.set(Screen::Menu);
    }
}

fn draw(
    mut commands: Commands,
    boot: Res<Boot>,
    card: Res<card::Card>,
    time: Res<Time<Real>>,
    look: Option<Res<look::Look>>,
    gm: Option<Res<crate::boards::Gm>>,
    mut labels: ResMut<look::Labels>,
    mut images: ResMut<Assets<Image>>,
    mut panels: Local<crate::boards::Panels>,
    mut slots: Query<(&look::Slot, &mut ImageNode, &mut Node, &mut Visibility)>,
) {
    // The bar fills across the hold in use, at the original's shape.
    let hold = crate::pace::hold(PICTURE_SECONDS, crate::pace::BOOT);
    let t = (time.elapsed_secs_f64() - boot.since) * PICTURE_SECONDS / hold;
    let mut pieces = Vec::new();
    if let Some(picture) = &boot.picture {
        pieces.push(Piece::new(picture.clone(), PICTURE_AT));
        let w = bar_width(t);
        if w > 0.0 {
            let mut bar = Piece::new(
                Handle::default(),
                Rect::from_corners(BAR_AT, BAR_AT + Vec2::new(w, BAR_HEIGHT)),
            );
            let [r, g, b] = BAR_COLOUR;
            bar.colour = Color::srgb_u8(r, g, b);
            pieces.push(bar);
        }
    }
    if let (Some(flow), Some(look), Some(gm)) = (card.flow.as_ref(), look.as_deref(), gm.as_deref())
    {
        pieces.extend(card::pieces(
            look,
            gm,
            &mut labels,
            &mut images,
            &mut panels,
            flow,
            0.0,
            (t * 60.0) as u64,
        ));
    }
    look::show(&mut commands, &mut slots, &pieces, Screen::Boot);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bar_fills_as_the_original_boot_measured() {
        assert_eq!(bar_width(0.0), 0.0);
        assert_eq!(bar_width(2.4), 0.0);
        assert!((bar_width(3.0) - 1.12).abs() < 1e-3);
        assert!((bar_width(4.16) - 1.12).abs() < 1e-3);
        // 8.17 a second between the modules' loads and the packages'.
        let slope = bar_width(10.0) - bar_width(9.0);
        assert!((slope - 8.17).abs() < 0.05, "{slope}");
        assert!((bar_width(12.54) - 69.58).abs() < 0.01);
        assert_eq!(bar_width(12.57), BAR_WIDTH);
        assert_eq!(bar_width(13.6), BAR_WIDTH);
        // Never past 0.95 of the whole before the packages are in.
        assert!(bar_width(12.55) <= 0.95 * BAR_WIDTH);
    }
}
