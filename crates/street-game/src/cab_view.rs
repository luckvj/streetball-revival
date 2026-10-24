//! Create Baller (`FUN_00273a08(screen, mode, gender)`: screen classes
//! 0x4a and 0x4b, the wizard for a man and a woman, mode 3; 0x4c the
//! premade picker, mode 1): `FUN_002d7720(screen, 0x3d, 3)`, layout 61
//! (the title, the L1 and R1 marks, the legend) over backdrop 3, and a
//! panel over it, each a layout of its own (`FUN_002748a8(panel, screen,
//! layout)`, built by `FUN_00274a40`): 63 the premade picker
//! (`FUN_00277be8`), 65 and 67..73 the wizard's steps. The 3D baller stands
//! in the backdrop's picture panel (`FUN_002db8b8`; the rewrite's
//! `ballers::preview`).
//!
//! `FUN_00274a40` gives the panels' pictures (kind 3) their sprites by
//! element id: the rows' bars 101.. (`XACOMMON` sheet 0, image 6, frame
//! 0), 120.. (frame 2), 100 (frame 1), 119 (frame 3); the tab icons 90..
//! (`XAFESCRN` sheet 0, image 7, by the table at `0x478e38`: 8, 6, 5, 4,
//! 2, 1, 7, 9), 130.. (frame 0x1b), 134.. (0x16), 142..145 (0x1d), 150..153
//! (0x1e), 146..149 (0x1c, stretched between 142 + k and 150 + k); the
//! stars 110..112 (`XACOMMON` image 1, frame 8) and 113..114 (frame 1),
//! 115 (`XAFESCRN` image 3, frame 0). Kind 2 is a filled box in prop 10's
//! colour, kind 4 words (prop 15 or 16, a text id), kind 20 a meter. Each
//! panel's enter lights its own tab (`FUN_00210048(tab, 2, 0, 7, frame)`,
//! the image's lit row, frames 10..19).
//!
//! The premade picker (`FUN_00277cf0`, input `FUN_00277fa8`, update
//! `FUN_00278098`): the screen's title becomes "Choose Baller" (0x2200),
//! its list 0x14 holds the six premade ballers by name (`FUN_001e6518`
//! loads each into the created player), L1 and R1 step it (actions 0x31,
//! 0x33: `FUN_0021ee10`), each step loads the premade into the 3D baller
//! and the read-outs; Cross asks "CREATE / %1S?" (`createplyr.txt` 14,
//! `FUN_00274260`: YES, EDIT, CANCEL); YES makes the career with it, EDIT
//! opens the wizard on it (panel 3), CANCEL stays (`FUN_002743d0`).
//!
//! The wizard ([`Step`]): Build, Look, Hair, Facial Hair (a man's only),
//! Outfit, Skills and Name (the first name's keyboard, the last name's,
//! then the icon). Every panel shares the base enter `FUN_00274950`: the
//! header (`FUN_002763c8` the first and last names in 0xfffbac00 on
//! elements 8 and 12, `FUN_002765c0` the height and weight on 13 and 17),
//! "Total: %1d DP" on element 3 (`FUN_002764c8`, `createplyr.txt` 10) and
//! "Item: %1d DP" on element 19 (`FUN_00276520`, 9: the focused row's
//! price, the panel's +0x1bc). A panel's lists are its layout's: list 0x14
//! (element 20) over the kind-10 rows 21.., list 0x2c (44) over 45..; their
//! words are made from the kind-19 templates 160 and 220 (`FUN_00275c58`,
//! `FUN_00276100`), the list arrows 4, 5 (0x14) and 6, 7 (0x2c) shown while
//! the list runs past its rows (`FUN_00274a40` hands them to the lists).

use bevy::prelude::*;
use street_assets::{
    db::Skill,
    fe,
    font::{Element, Layout},
};
use street_sim::story::CreatedBaller;

use crate::{
    choose::{place, spot},
    look::{self, Look, Piece, colours, fonts},
    setup::Disc,
};

/// The premade picker's panel layout.
pub const PREMADE: u32 = 63;
/// The tab icons' frames (`0x478e38`).
const TAB_FRAMES: [usize; 8] = [8, 6, 5, 4, 2, 1, 7, 9];

/// The sprites `FUN_00274a40` hands the panels.
#[derive(Resource, Default)]
pub struct CabArt {
    /// `XACOMMON` sheet 0, image 6, frames 0..3.
    bars: Vec<Option<(Handle<Image>, Vec2)>>,
    /// `XACOMMON` sheet 0, image 1, frames 1 and 8.
    stars: [Option<(Handle<Image>, Vec2)>; 2],
    /// `XAFESCRN` sheet 0, image 7, by frame.
    icons: Vec<Option<(Handle<Image>, Vec2)>>,
    /// `XAFESCRN` sheet 0, image 3, frame 0 (element 115).
    big_star: Option<(Handle<Image>, Vec2)>,
    /// `XACOMMON` sheet 0 by (image, frame): image 6 frames 0..=9 (the
    /// bars) and image 1 frames 0..=8 (the stars), for the panels whose
    /// base hands out others than `FUN_00274a40`'s ([`Pic::Common`]).
    common: Vec<((usize, usize), (Handle<Image>, Vec2))>,
}

/// Startup: crop the sprites.
pub fn start(mut commands: Commands, mut disc: ResMut<Disc>, mut images: ResMut<Assets<Image>>) {
    let mut art = CabArt::default();
    let common = fe::read(&mut disc.iso, "/FE/XACOMMON.BIN").ok();
    let screens = fe::read(&mut disc.iso, "/FE/XAFESCRN.BIN").ok();
    let mut add = |t: Option<street_assets::ssh::Texture>| {
        t.map(|t| {
            let size = Vec2::new(t.width as f32, t.height as f32);
            (images.add(look::image(t.width, t.height, t.rgba)), size)
        })
    };
    if let Some(sheet) = common.as_ref().and_then(|s| s.first()) {
        art.bars = (0..4)
            .map(|f| add(look::sprite_crop(sheet, 6, f)))
            .collect();
        art.stars = [
            add(look::sprite_crop(sheet, 1, 1)),
            add(look::sprite_crop(sheet, 1, 8)),
        ];
        for (image, frames) in [(6usize, 0..=9usize), (1, 0..=8)] {
            for frame in frames {
                if let Some(sprite) = add(look::sprite_crop(sheet, image, frame)) {
                    art.common.push(((image, frame), sprite));
                }
            }
        }
    }
    if let Some(sheet) = screens.as_ref().and_then(|s| s.first()) {
        art.icons = (0..0x20)
            .map(|f| add(look::sprite_crop(sheet, 7, f)))
            .collect();
        art.big_star = add(look::sprite_crop(sheet, 3, 0));
    }
    commands.insert_resource(art);
}

impl CabArt {
    /// The sprite `FUN_00274a40` gives element `id`, if any.
    fn sprite(&self, id: u32) -> Option<&(Handle<Image>, Vec2)> {
        let bar = |f: usize| self.bars.get(f).and_then(Option::as_ref);
        match id {
            90..=97 => self.icon(TAB_FRAMES[(id - 90) as usize]),
            100 => bar(1),
            101..=109 => bar(0),
            110..=112 => self.stars[1].as_ref(),
            113..=114 => self.stars[0].as_ref(),
            115 => self.big_star.as_ref(),
            119 => bar(3),
            120..=129 => bar(2),
            130..=133 => self.icon(0x1b),
            134..=141 => self.icon(0x16),
            142..=145 => self.icon(0x1d),
            146..=149 => self.icon(0x1c),
            150..=153 => self.icon(0x1e),
            _ => None,
        }
    }

    fn icon(&self, frame: usize) -> Option<&(Handle<Image>, Vec2)> {
        self.icons.get(frame).and_then(Option::as_ref)
    }

    fn common(&self, image: usize, frame: usize) -> Option<&(Handle<Image>, Vec2)> {
        self.common
            .iter()
            .find(|(at, _)| *at == (image, frame))
            .map(|(_, sprite)| sprite)
    }
}

/// A picture (kind 3) a panel's base or its lists give in place of
/// `FUN_00274a40`'s.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pic {
    /// `XACOMMON` sheet 0, (image, frame).
    Common(usize, usize),
    /// A list's arrow: 0 up, 1 down, 2 left, 3 right.
    Arrow(u8),
    /// Nothing (an element its base gives no picture).
    None,
}

/// A list row's words: drawn at the kind-10 row `row`'s point in its
/// template's font and anchor (kind 19: 160 for list 0x14, 220 for 0x2c),
/// the row widget's colours (`FUN_0021c590`: 0xff88a0be, lit 0xfffbac00).
pub struct RowWords {
    pub row: u32,
    pub template: u32,
    pub words: String,
    pub lit: bool,
}

/// What a panel shows besides its layout's own pieces.
#[derive(Default)]
pub struct Values {
    /// Words by element id (in place of the layout's own words there).
    pub words: Vec<(u32, String)>,
    /// Meters (kind 20) by element id, 0..6.
    pub meters: Vec<(u32, u8)>,
    /// Meters at a list row's point (the skills panel's column 1).
    pub row_meters: Vec<(u32, u8)>,
    /// Pictures (kind 3) left out (the lists' arrows while they do not
    /// scroll, the rows a woman's build list leaves).
    pub hidden: Vec<u32>,
    /// Pictures shown with another frame of `XAFESCRN` image 7 (the
    /// panel's lit tab).
    pub frames: Vec<(u32, usize)>,
    /// The lists' rows.
    pub rows: Vec<RowWords>,
    /// Colour boxes over list 0x2c's rows (`FUN_00275f68`, 19 by 24: the
    /// hair and facial hair colours), sRGB.
    pub swatches: Vec<(u32, [u8; 3])>,
    /// Filled boxes drawn over the panel (the height list's cursor).
    pub boxes: Vec<(Rect, [u8; 4])>,
    /// Pictures from elsewhere (the icons' portrait tiles), by the image,
    /// the box and the brightness.
    pub tiles: Vec<(Handle<Image>, Rect, f32)>,
    /// Right arrows centred off a list row's point.
    pub arrows: Vec<(u32, Vec2)>,
    /// Pictures given by the panel's own base or lists ([`Pic`]).
    pub pics: Vec<(u32, Pic)>,
    /// The help line (kind 5, element 1), when the panel has one.
    pub help: Option<String>,
    /// List rows drawn at a dimmed list's 0x9f (`+0x124`).
    pub dim_rows: Vec<u32>,
}

/// A panel layout's pieces (`FUN_00274a40`'s build): the boxes, the
/// pictures, the static words, then the values.
pub fn panel_pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &CabArt,
    gm: &crate::boards::Gm,
    layout: &Layout,
    values: &Values,
) -> Vec<Piece> {
    let mut out = Vec::new();
    // Back to front: the layout lists its pieces front first (the labels
    // before the bars they sit on, the panes last).
    for e in layout.elements.iter().rev() {
        let (Some((x, y)), size) = (e.position(), e.size()) else {
            continue;
        };
        let at = Vec2::new(f32::from(x), f32::from(y));
        match e.kind {
            2 if !values.hidden.contains(&e.id) => {
                let Some((w, h)) = size else { continue };
                let mut p = Piece::new(
                    gm.white.clone(),
                    Rect::from_corners(at, at + Vec2::new(f32::from(w), f32::from(h))),
                );
                let c = e
                    .props
                    .get(&10)
                    .map_or([0, 0, 0, 0xff], |&c| street_assets::font::argb(c));
                p.colour = Color::srgba_u8(c[0], c[1], c[2], c[3]);
                out.push(p);
            }
            3 if !values.hidden.contains(&e.id) => {
                let pic = values
                    .pics
                    .iter()
                    .find(|(id, _)| *id == e.id)
                    .map(|&(_, p)| p);
                let sprite = match (pic, values.frames.iter().find(|(id, _)| *id == e.id)) {
                    (Some(Pic::Common(image, frame)), _) => art.common(image, frame),
                    (Some(Pic::Arrow(_) | Pic::None), _) => None,
                    (None, Some(&(_, frame))) => art.icon(frame),
                    (None, None) => art.sprite(e.id),
                };
                if let Some((image, size)) = sprite {
                    out.push(Piece::new(image.clone(), anchored(at, *size, e.align)));
                } else if pic != Some(Pic::None)
                    && let Some(arrow) = arrow(look, e, pic)
                {
                    out.push(arrow);
                }
            }
            5 if e.id == 1 => {
                if let Some(help) = values.help.as_deref() {
                    write(&mut out, labels, look, images, e, help);
                }
            }
            4 if !values.words.iter().any(|(id, _)| *id == e.id)
                && !values.hidden.contains(&e.id) =>
            {
                // Prop 15 or 16, whichever is set (0 is none).
                let id = [15, 16]
                    .into_iter()
                    .filter_map(|k| e.props.get(&k).copied())
                    .find(|&id| id != 0)
                    .unwrap_or(0);
                if id != 0
                    && let Some(words) = look.text_full(id as u16)
                {
                    write(&mut out, labels, look, images, e, words);
                }
            }
            _ => {}
        }
    }
    for (id, rgb) in &values.swatches {
        if let Some(at) = element(layout, 10, *id).and_then(spot) {
            let mut p = Piece::new(
                gm.white.clone(),
                Rect::from_corners(at, at + Vec2::new(19.0, 24.0)),
            );
            // The colour boxes draw at the GS's half brightness (0x40).
            p.colour = Color::srgb_u8(rgb[0] / 2, rgb[1] / 2, rgb[2] / 2);
            out.push(p);
        }
    }
    for (rect, c) in &values.boxes {
        let mut p = Piece::new(gm.white.clone(), *rect);
        p.colour = Color::srgba_u8(c[0], c[1], c[2], c[3]);
        out.push(p);
    }
    for (image, rect, bright) in &values.tiles {
        let mut p = Piece::new(image.clone(), *rect);
        p.colour = Color::srgb(*bright, *bright, *bright);
        out.push(p);
    }
    let Some(pm) = look.prematch.as_ref() else {
        return out;
    };
    for &(id, level) in &values.meters {
        if let Some(e) = element(layout, 20, id)
            && let Some(at) = spot(e)
        {
            out.extend(crate::chooseteam::meter(pm, at, level));
        }
    }
    for &(row, off) in &values.arrows {
        if let Some(at) = element(layout, 10, row).and_then(spot) {
            out.push(Piece::new(
                pm.arrow_right.clone(),
                Rect::from_center_size(at + off, pm.arrow_side_size),
            ));
        }
    }
    for &(row, level) in &values.row_meters {
        if let Some(at) = element(layout, 10, row).and_then(spot) {
            out.extend(crate::chooseteam::meter(pm, at, level));
        }
    }
    for r in &values.rows {
        let (Some(row), Some(template)) =
            (element(layout, 10, r.row), element(layout, 19, r.template))
        else {
            continue;
        };
        let Some(at) = spot(row) else { continue };
        let font = template
            .props
            .get(&13)
            .map_or(fonts::LIST_SMALL, |&f| f as usize);
        let mut colour = if r.lit {
            colours::ROW_LIT
        } else {
            colours::ROW
        };
        if values.dim_rows.contains(&r.row) {
            for c in &mut colour[..3] {
                *c = (f32::from(*c) * 0x9f as f32 / 255.0) as u8;
            }
        }
        place(
            &mut out,
            labels,
            look,
            images,
            font,
            &r.words,
            colour,
            at,
            template.align,
        );
    }
    for (id, words) in &values.words {
        // The words' own element: the first kind 4 of the id whose colour
        // is the layout's lit one (the panels list a lit and a plain copy).
        if let Some(e) = element(layout, 4, *id) {
            write(&mut out, labels, look, images, e, words);
        }
    }
    out
}

/// A layout's element of `kind` and `id`.
fn element(layout: &Layout, kind: u8, id: u32) -> Option<&Element> {
    layout
        .elements
        .iter()
        .find(|e| e.kind == kind && e.id == id)
}

/// The lists' arrows (elements 4..7, which `FUN_00274a40` hands to the
/// lists, or a panel's own by [`Pic::Arrow`]): the screens' orange arrows,
/// up, down, left and right.
fn arrow(look: &Look, e: &Element, pic: Option<Pic>) -> Option<Piece> {
    let pm = look.prematch.as_ref()?;
    let at = spot(e)?;
    let way = match (pic, e.id) {
        (Some(Pic::Arrow(way)), _) => way,
        (_, 4..=7) => (e.id - 4) as u8,
        _ => return None,
    };
    let (image, size) = match way {
        0 => (&pm.arrow_up, pm.arrow_size),
        1 => (&pm.arrow_down, pm.arrow_size),
        2 => (&pm.arrow_left, pm.arrow_side_size),
        _ => (&pm.arrow_right, pm.arrow_side_size),
    };
    Some(Piece::new(image.clone(), anchored(at, size, e.align)))
}

/// A picture's box by the element's anchor (`align & 3` across, `(align
/// >> 2) & 3` down).
fn anchored(at: Vec2, size: Vec2, align: u8) -> Rect {
    let dx = match align & 3 {
        1 => size.x / 2.0,
        2 => size.x,
        _ => 0.0,
    };
    let dy = match (align >> 2) & 3 {
        1 => size.y / 2.0,
        2 => size.y,
        _ => 0.0,
    };
    let min = at - Vec2::new(dx, dy);
    Rect::from_corners(min, min + size)
}

fn write(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    look: &Look,
    images: &mut Assets<Image>,
    e: &Element,
    words: &str,
) {
    let Some(at) = spot(e) else { return };
    let font = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
    let colour = e
        .props
        .get(&10)
        .map_or(colours::CREAM, |&c| street_assets::font::argb(c));
    place(out, labels, look, images, font, words, colour, at, e.align);
}

/// The header's height and weight (`FUN_001d6440`, `FUN_001d62a8`).
fn size_words(height_in: i16, weight_lb: i16) -> [(u32, String); 2] {
    [
        (13, format!("{}'{}\"", height_in / 12, height_in % 12)),
        (17, format!("{weight_lb} lbs")),
    ]
}

/// "%1d DP"-style counts print grouped by thousands (the front end's
/// `%1d`: "Total: 1,500 DP").
fn dp_words(look: &Look, file: &str, index: usize, plain: &str, n: i64) -> String {
    let count = if n < 0 {
        format!(
            "-{}",
            crate::users::count((-n).min(i64::from(u32::MAX)) as u32)
        )
    } else {
        crate::users::count(n.min(i64::from(u32::MAX)) as u32)
    };
    look.text(file, index)
        .unwrap_or(plain)
        .replace("%1d", &count)
}

/// The premade picker's values for premade `record`: its first and last
/// names (elements 8, 12) and size (13, 17) on the left, its seven skill
/// bars on the meters (62..68), the total (3).
pub fn premade_values(look: &Look, disc: &Disc, record: usize) -> Values {
    let mut v = Values {
        // The list deletes its arrows (elements 4, 5) to draw them itself;
        // element 160's "Choose Baller" (0x2200) goes to the screen's
        // title (`FUN_00277cf0`).
        hidden: vec![4, 5, 160],
        ..Default::default()
    };
    let Some(p) = disc.players.get(record) else {
        return v;
    };
    v.words.push((8, p.first_name.clone()));
    v.words.push((12, p.last_name.clone()));
    v.words.extend(size_words(p.height_in, p.weight_lb));
    v.words
        .push((3, dp_words(look, "createplyr.txt", 10, "Total: %1d DP", 0)));
    for (k, skill) in Skill::ALL.into_iter().enumerate() {
        let bar = crate::progression_trace::skill_bar(skill, p.ratings[skill.rating()]);
        v.meters.push((62 + k as u32, bar.min(5)));
    }
    v
}

/// The wizard's steps: `FUN_00273f58`'s panel slots 3 (Build, layout 65,
/// `FUN_00276cc8`), 5 (Look, 67, `FUN_0027b908`), 6 (Hair, 68,
/// `FUN_0027a7e8`), 7 (Facial Hair, 69, `FUN_00279aa0`), 8 (Outfit, 70,
/// `FUN_00278748`), 9 (Skills, 71, `FUN_0027cfe8`) and 10 (Name, 72,
/// `FUN_0027bfa0`: the first name's keyboard, the last name's, then the
/// icon). Slot 11 (layout 73, `FUN_00279628`, "Is This Your Baller?") is
/// not on the way: the name panel's next asks "CREATE %1S?" (`FUN_0027c088`
/// → `FUN_00274260`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Step {
    Build,
    Look,
    Hair,
    Facial,
    Outfit,
    Skills,
    First,
    Last,
    Icon,
}

/// The steps in order.
pub const STEPS: [Step; 9] = [
    Step::Build,
    Step::Look,
    Step::Hair,
    Step::Facial,
    Step::Outfit,
    Step::Skills,
    Step::First,
    Step::Last,
    Step::Icon,
];

/// How the model viewer frames the baller (`FUN_002dc0e0` by the viewer's
/// +0x88, set through the screen's +0xf0 states `FUN_002745d8` steps).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Framing {
    /// +0x88 1 (`FUN_002dc4e0`, pose 0x477; the screen's states 7, 8, 4:
    /// `FUN_00274808`): the whole baller, the camera 76.13 away and 4.29
    /// above the hips, level.
    Body,
    /// +0x88 0 (`FUN_002dc4a8`, pose 0x476; states 5, 6, 3:
    /// `FUN_002747b8`): the head, the camera level with bone 21, 26.74
    /// away (23.74 less the zoom, held at -3).
    Head,
    /// +0x88 3 (`FUN_002dc560`, pose 0x477; states 2, 0, 1:
    /// `FUN_00274858`, the premade picker): chest up, 36.7 away and 6.18
    /// above the hips, level.
    Picker,
}

impl Step {
    pub fn at(index: usize) -> Step {
        STEPS[index.min(STEPS.len() - 1)]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    /// The panel's layout (`FUN_002748a8`'s third argument).
    pub fn layout(self) -> u32 {
        match self {
            Step::Build => 65,
            Step::Look => 67,
            Step::Hair => 68,
            Step::Facial => 69,
            Step::Outfit => 70,
            Step::Skills => 71,
            Step::First | Step::Last | Step::Icon => 72,
        }
    }

    /// The panel's tab (elements 90..96) and the lit frame its enter gives
    /// it (`FUN_00277070` 0x12, `FUN_0027baf0` 0xd, `FUN_0027aa50` 10,
    /// `FUN_00279d40` 0x10, `FUN_00278bc8` 0xe, `FUN_0027d360` 0xb; the
    /// name panel the lit "A", 0x11).
    pub fn tab(self) -> (u32, usize) {
        match self {
            Step::Build => (90, 0x12),
            Step::Look => (91, 0xd),
            Step::Hair => (92, 10),
            Step::Facial => (93, 0x10),
            Step::Outfit => (94, 0xe),
            Step::Skills => (95, 0xb),
            Step::First | Step::Last | Step::Icon => (96, 0x11),
        }
    }

    /// Look, Hair and Facial Hair close on the head (`FUN_002747b8`); the
    /// rest show the whole baller (`FUN_00274808`).
    pub fn framing(self) -> Framing {
        match self {
            Step::Look | Step::Hair | Step::Facial => Framing::Head,
            _ => Framing::Body,
        }
    }

    /// The panel's next (each panel's +0x164; Hair's `FUN_0027a8f0` skips
    /// Facial Hair for a woman, field 12 set). The icon's next asks.
    pub fn next(self, female: bool) -> Option<Step> {
        Some(match self {
            Step::Build => Step::Look,
            Step::Look => Step::Hair,
            Step::Hair if female => Step::Outfit,
            Step::Hair => Step::Facial,
            Step::Facial => Step::Outfit,
            Step::Outfit => Step::Skills,
            Step::Skills => Step::First,
            Step::First => Step::Last,
            Step::Last => Step::Icon,
            Step::Icon => return None,
        })
    }

    /// The panel's back (+0x16c: `FUN_0027ba50` 3, `FUN_0027a970` 5,
    /// `FUN_00279be8` 6, `FUN_00278a48` 7 or 6 for a woman, `FUN_0027d130`
    /// 8, `FUN_0027c0c8` 9); Build's leaves the screen.
    pub fn back(self, female: bool) -> Option<Step> {
        Some(match self {
            Step::Build => return None,
            Step::Look => Step::Build,
            Step::Hair => Step::Look,
            Step::Facial => Step::Hair,
            Step::Outfit if female => Step::Hair,
            Step::Outfit => Step::Facial,
            Step::Skills => Step::Outfit,
            Step::First => Step::Skills,
            Step::Last => Step::First,
            Step::Icon => Step::Last,
        })
    }

    /// The L1 mark shows past the first panel, R1 before the name's
    /// (`FUN_00277070` hides elements 0x9e and 0x9d, the name panel 0x9c
    /// and 0x9b).
    pub fn marks(self) -> (bool, bool) {
        (
            self != Step::Build,
            !matches!(self, Step::First | Step::Last | Step::Icon),
        )
    }
}

/// Edit Baller's panels (`FUN_0028b930` builds them over `FUN_0028c798`,
/// the editor's panel base): Edit Outfit (`FUN_0028e190`, layout 80),
/// Edit Shoes (`FUN_00295ab8`, 81), Edit Hair (`FUN_0028ff18`, 82), Edit
/// Facial Hair (`FUN_0028f4f8`, 83) and Edit Skills (`FUN_002966f8`, 84);
/// L1 and R1 go round them (a woman's skip Facial Hair). The panels' list
/// templates are 152 and 210, the weight is element 14, the tabs 90..94
/// take `0x4846e8`'s frames (2, 3, 5, 4, 1), each panel's enter its own
/// lit one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Edit {
    Outfit,
    Shoes,
    Hair,
    Facial,
    Skills,
}

/// The editor's panels in order.
pub const EDITS: [Edit; 5] = [
    Edit::Outfit,
    Edit::Shoes,
    Edit::Hair,
    Edit::Facial,
    Edit::Skills,
];

/// The editor's tabs' frames (`0x4846e8`).
const EDIT_TABS: [usize; 5] = [2, 3, 5, 4, 1];

impl Edit {
    pub fn at(index: usize) -> Edit {
        EDITS[index.min(EDITS.len() - 1)]
    }

    pub fn index(self) -> usize {
        self as usize
    }

    pub fn layout(self) -> u32 {
        80 + self as u32
    }

    /// The panel's tab and the lit frame its enter gives it
    /// (`FUN_0028e2f8` 0xe, `FUN_00295c08` 0xf, `FUN_0028ff90` 10,
    /// `FUN_0028f570` 0x10, `FUN_002968d0` 0xb).
    pub fn tab(self) -> (u32, usize) {
        match self {
            Edit::Outfit => (90, 0xe),
            Edit::Shoes => (91, 0xf),
            Edit::Hair => (92, 10),
            Edit::Facial => (93, 0x10),
            Edit::Skills => (94, 0xb),
        }
    }

    /// The wizard's panel whose lists it shares (the shoes have none).
    pub fn step(self) -> Option<Step> {
        match self {
            Edit::Outfit => Some(Step::Outfit),
            Edit::Shoes => None,
            Edit::Hair => Some(Step::Hair),
            Edit::Facial => Some(Step::Facial),
            Edit::Skills => Some(Step::Skills),
        }
    }

    pub fn framing(self) -> Framing {
        match self {
            Edit::Hair | Edit::Facial => Framing::Head,
            _ => Framing::Body,
        }
    }

    /// R1 (`dir` 1) and L1 (-1) go round the panels, past Facial Hair for
    /// a woman.
    pub fn turn(self, dir: i32, female: bool) -> Edit {
        let mut at = self.index() as i32;
        loop {
            at = (at + dir).rem_euclid(EDITS.len() as i32);
            let to = EDITS[at as usize];
            if !(female && to == Edit::Facial) {
                return to;
            }
        }
    }
}

/// A question Create Baller or Edit Baller asks over its panel.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CabAsk {
    /// Build's Triangle (`FUN_00276f08`): "Are you sure you want to exit
    /// Create Baller? All your changes will be lost." (`createplyr.txt` 6,
    /// `FUN_00274208(screen, 1)`), or "...back up?..." (`createplyr_build.txt`
    /// 15, `FUN_00275838`) when the premade picker opened the wizard (its
    /// enter, `FUN_00277cf0`, sets the screen's +0x138).
    Leave { back_up: bool },
    /// The wizard's Build or Outfit past the points (`FUN_002758a0`:
    /// `createplyr.txt` 8, 250 wide, Continue).
    WizardPoor,
    /// Edit Baller's Cross on an outfit or shoes not owned
    /// (`FUN_0028e710`, `FUN_00295e40`): "Use %1d development points\n to
    /// unlock %2s?" (`editplyr.txt` 3, `FUN_0025d388`: Yes, No).
    Buy { cost: u32, name: String },
    /// ...or when the points do not cover it: "You do not have enough
    /// development points to unlock %2s." (5, `FUN_0025d660`, 200 wide).
    Poor { name: String },
    /// Edit Baller's Start ("Exit", `FUN_0028c0b8`): "Are you sure you
    /// want\nto exit Edit Baller?" (9, Yes, No).
    Exit,
}

impl CabAsk {
    /// The words and the width they wrap to (0: as written).
    pub fn words(&self, look: &Look) -> (String, f32) {
        let text = |file: &str, k: usize, or: &str| look.text(file, k).unwrap_or(or).to_owned();
        match self {
            CabAsk::Leave { back_up: false } => (
                text(
                    "createplyr.txt",
                    6,
                    "Are you sure you want\\nto exit Create Baller?\\nAll your changes will be lost.",
                ),
                0.0,
            ),
            CabAsk::Leave { back_up: true } => (
                text(
                    "createplyr_build.txt",
                    15,
                    "Are you sure you want\\nto back up?\\nAll your changes will be lost.",
                ),
                0.0,
            ),
            CabAsk::WizardPoor => (
                text(
                    "createplyr.txt",
                    8,
                    "You do not have enough development points to unlock this item.",
                ),
                250.0,
            ),
            CabAsk::Buy { cost, name } => (
                text(
                    "editplyr.txt",
                    3,
                    "Use %1d development points\\n to unlock %2s?",
                )
                .replace("%1d", &crate::users::count(*cost))
                .replace("%2s", name),
                0.0,
            ),
            CabAsk::Poor { name } => (
                text(
                    "editplyr.txt",
                    5,
                    "You do not have enough development points to unlock %2s.",
                )
                .replace("%2s", name),
                200.0,
            ),
            CabAsk::Exit => (
                text(
                    "editplyr.txt",
                    9,
                    "Are you sure you want\\nto exit Edit Baller?",
                ),
                0.0,
            ),
        }
    }

    /// Yes and No (`common.txt` 2, 3), or Continue (20).
    pub fn rows(&self, look: &Look) -> Vec<String> {
        let text = |k: usize, or: &str| look.text("common.txt", k).unwrap_or(or).to_owned();
        match self {
            CabAsk::WizardPoor | CabAsk::Poor { .. } => vec![text(20, "Continue")],
            _ => vec![text(2, "Yes"), text(3, "No")],
        }
    }

    /// The row lit when it opens (`FUN_0025cda0(dialog, 1)`: No, or the
    /// one row).
    pub fn opening_row(&self) -> usize {
        match self {
            CabAsk::WizardPoor | CabAsk::Poor { .. } => 0,
            _ => 1,
        }
    }
}

/// The shoes' list (`editplyr_shoes.txt` 2..21, "Style A".. in price
/// order, `street_sim::cab::SHOES`) and the row worn.
pub fn shoe_list(look: &Look, b: &CreatedBaller) -> (Vec<String>, usize) {
    (
        (0..street_sim::cab::SHOES.len())
            .map(|k| {
                look.text("editplyr_shoes.txt", 2 + k)
                    .unwrap_or("Style")
                    .trim()
                    .to_owned()
            })
            .collect(),
        street_sim::cab::shoe_row(b.shoes),
    )
}

/// The rows list 0x14 holds and the first shown: `count` rows through a
/// window of `shown` (the layout's kind-10 rows), the cursor kept inside.
pub fn scrolled(top: usize, cursor: usize, count: usize, shown: usize) -> usize {
    if count <= shown {
        return 0;
    }
    let top = top.min(count - shown);
    if cursor < top {
        cursor
    } else if cursor >= top + shown {
        cursor + 1 - shown
    } else {
        top
    }
}

/// The hair and facial hair colours (`0x3f8828`, eight; `0x3f8680`, the
/// first six), 0xffRRGGBB.
pub const HAIR_COLOURS: [u32; 8] = [
    0xff000000, 0xff34190f, 0xff463124, 0xff644736, 0xff612215, 0xffa67548, 0xff055508, 0xff0e1984,
];

/// The special outfits' names, codes 54..81: their strings in
/// `editplyr_clothes.txt` by the per-code table at `0x3f94c8`.
pub const SPECIAL_OUTFIT_TEXT: [usize; 28] = [
    84, 80, 69, 61, 62, 82, 72, 74, 68, 76, 63, 71, 75, 60, 67, 77, 73, 83, 85, 78, 81, 64, 70, 66,
    79, 65, 86, 87,
];

/// What a wizard step lists: the rows of list 0x14 by their words, and the
/// row the baller's value is on.
pub fn list(
    look: &Look,
    b: &CreatedBaller,
    step: Step,
    extras: &crate::ballers::OutfitExtras,
) -> (Vec<String>, usize) {
    let text = |file: &str, k: usize, or: &str| look.text(file, k).unwrap_or(or).trim().to_owned();
    let build = usize::from(b.build.min(5));
    let (looks, hairs, beards) = street_assets::db::kit_limits(build);
    match step {
        // `0x479418` (Stocky, Athletic, Lean, Massive) or `0x479420` (Fit,
        // Athletic): `createplyr_build.txt` 7..10, 12, 13.
        Step::Build => {
            if b.female {
                (
                    [12, 13]
                        .map(|k| text("createplyr_build.txt", k, "Build"))
                        .to_vec(),
                    build.saturating_sub(4).min(1),
                )
            } else {
                (
                    (7..=10)
                        .map(|k| text("createplyr_build.txt", k, "Build"))
                        .collect(),
                    build.min(3),
                )
            }
        }
        // "Look A".. (`createplyr_look.txt` 3.. for a man, 21.. a woman).
        Step::Look => {
            let first = if b.female { 21 } else { 3 };
            (
                (0..looks)
                    .map(|k| text("createplyr_look.txt", first + k, "Look"))
                    .collect(),
                crate::ballers::look_row(b).min(looks.max(1) - 1),
            )
        }
        // Bald, then the styles (`editplyr_hair.txt` 2.., 20.. a woman's).
        Step::Hair => {
            let first = if b.female { 20 } else { 2 };
            (
                (0..=hairs)
                    .map(|k| text("editplyr_hair.txt", first + k, "Hair"))
                    .collect(),
                (i32::from(b.hair_style) + 1).clamp(0, hairs as i32) as usize,
            )
        }
        // None, then the styles (`editplyr_facial.txt` 2..).
        Step::Facial => (
            (0..=beards)
                .map(|k| text("editplyr_facial.txt", 2 + k, "Facial Hair"))
                .collect(),
            (i32::from(b.facial_hair_style) + 1).clamp(0, beards as i32) as usize,
        ),
        // The build's nine outfits (`editplyr_clothes.txt` 6 + 9 * build..)
        // and the specials by their codes' names (`0x3f94c8`).
        Step::Outfit => {
            let mut kit = 0;
            (
                crate::ballers::outfit_list(b, extras)
                    .into_iter()
                    .map(
                        |code| match code.checked_sub(street_sim::cab::SPECIAL_OUTFIT) {
                            Some(k) => text(
                                "editplyr_clothes.txt",
                                SPECIAL_OUTFIT_TEXT
                                    .get(usize::from(k))
                                    .copied()
                                    .unwrap_or(87),
                                "Outfit",
                            ),
                            None => {
                                kit += 1;
                                text("editplyr_clothes.txt", 6 + 9 * build + kit - 1, "Outfit")
                            }
                        },
                    )
                    .collect(),
                crate::ballers::outfit_index(b, extras),
            )
        }
        // The seven skills (`common_fe.txt` 27..33, as layout 63 names them).
        Step::Skills => (
            (0..7)
                .map(|k| look.text_id(0x11b + k as u16).unwrap_or("Skill").to_owned())
                .collect(),
            0,
        ),
        Step::First | Step::Last | Step::Icon => (Vec::new(), 0),
    }
}

/// The kind-10 rows of list `list` (element 20 or 44) in a layout, by
/// their place in the list (prop 34, the row's index).
fn list_rows(layout: &Layout, list: u32) -> Vec<u32> {
    let mut rows: Vec<(u32, u32)> = layout
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&list))
        .map(|e| (e.props.get(&34).copied().unwrap_or(0), e.id))
        .collect();
    rows.sort_unstable();
    rows.into_iter().map(|(_, id)| id).collect()
}

/// Everything a wizard panel reads from the baller being made.
pub struct Wizard<'a> {
    pub baller: &'a CreatedBaller,
    /// The outfit list's rows round the nine.
    pub extras: &'a crate::ballers::OutfitExtras,
    pub step: Step,
    /// List 0x14's first row shown.
    pub top: usize,
    /// The skills panel's focused row.
    pub skill: usize,
    /// The focused item's price (the panel's +0x1bc).
    pub item: i64,
    /// Development points left ("Total").
    pub total: i64,
    /// The skills' next steps' prices, `None` at the sixth crown.
    pub skill_prices: [Option<u32>; 7],
    /// The icons' first shown.
    pub icon_top: usize,
    /// Edit Baller's panel, when the panel is the editor's.
    pub edit: Option<Edit>,
}

/// The wizard panel's values: the header, the lists, the read-outs.
pub fn wizard_values(
    look: &Look,
    layout: &Layout,
    w: &Wizard,
    portraits: Option<&mut crate::portraits::Portraits>,
    images: &mut Assets<Image>,
) -> Values {
    let b = w.baller;
    let mut v = Values::default();
    // The lists' templates (the editor's layouts name them 152 and 210).
    let (t0, t1) = if w.edit.is_some() {
        (152, 210)
    } else {
        (160, 220)
    };
    match w.edit {
        Some(edit) => {
            for (k, &frame) in EDIT_TABS.iter().enumerate() {
                v.frames.push((90 + k as u32, frame));
            }
            let (tab, frame) = edit.tab();
            v.frames.retain(|(id, _)| *id != tab);
            v.frames.push((tab, frame));
        }
        None => {
            let (tab, frame) = w.step.tab();
            v.frames.push((tab, frame));
        }
    }
    v.words.push((8, b.first_name.clone()));
    v.words.push((12, b.last_name.clone()));
    v.words.extend(size_words(
        i16::from(b.height_in),
        b.weight_lb.min(999) as i16,
    ));
    // The editor's layouts put the weight on element 14.
    if let Some((_, weight)) = v.words.iter().find(|(id, _)| *id == 17).cloned() {
        v.words.push((14, weight));
    }
    v.words.push((
        3,
        dp_words(look, "createplyr.txt", 10, "Total: %1d DP", w.total),
    ));
    if element(layout, 4, 19).is_some() {
        v.words.push((
            19,
            dp_words(look, "createplyr.txt", 9, "Item: %1d DP", w.item),
        ));
    }
    let rows = list_rows(layout, 20);
    let (items, at) = if w.edit == Some(Edit::Shoes) {
        shoe_list(look, b)
    } else {
        list(look, b, w.step, w.extras)
    };
    match w.step {
        Step::Skills => skills_rows(look, layout, w, &mut v, (t0, t1)),
        Step::First | Step::Last | Step::Icon => {}
        _ => {
            let top = scrolled(w.top, at, items.len(), rows.len());
            for (k, &row) in rows.iter().enumerate() {
                let Some(words) = items.get(top + k) else {
                    break;
                };
                v.rows.push(RowWords {
                    row,
                    template: t0,
                    words: words.clone(),
                    lit: top + k == at,
                });
            }
            // A woman's two builds leave the rows under them to the
            // panel's box 18 (`FUN_00277070`: bars 103.. hidden).
            if w.step == Step::Build {
                if b.female {
                    v.hidden.extend(103..110);
                } else {
                    v.hidden.push(18);
                }
            }
            if top == 0 {
                v.hidden.push(4);
            }
            if top + rows.len() >= items.len() {
                v.hidden.push(5);
            }
        }
    }
    let side = list_rows(layout, 44);
    match w.step {
        // M, L, XL, XXL (`0x479410`: `createplyr_build.txt` 3..6) on the
        // template 220; the cursor box on the size's block
        // (`FUN_0021b2f8`, 62 wide and 0x29, 0x32, 0x3d, 0x49 tall from
        // `0x479400`, 5 and 9 inside element 142 + size).
        Step::Build => {
            let size = crate::ballers::size_of(b);
            for (k, &row) in side.iter().enumerate() {
                v.rows.push(RowWords {
                    row,
                    template: 220,
                    words: look
                        .text("createplyr_build.txt", 3 + k)
                        .unwrap_or(crate::ballers::SIZE_NAMES[k.min(3)])
                        .to_owned(),
                    lit: k == size,
                });
            }
            const TALL: [f32; 4] = [41.0, 50.0, 61.0, 73.0];
            if let Some(at) = element(layout, 3, 142 + size as u32).and_then(spot) {
                let min = at + Vec2::new(5.0, 9.0);
                v.boxes.push((
                    Rect::from_corners(min, min + Vec2::new(62.0, TALL[size.min(3)] - 9.0)),
                    [0, 0, 0, 0x50],
                ));
            }
            v.hidden.extend([6, 7]);
        }
        Step::Hair | Step::Facial => {
            let n = if w.step == Step::Hair { 8 } else { 6 };
            for (k, &row) in side.iter().take(n).enumerate() {
                let c = HAIR_COLOURS[k];
                v.swatches
                    .push((row, [(c >> 16) as u8, (c >> 8) as u8, c as u8]));
            }
        }
        // Color A, B, C (`editplyr_clothes.txt` 3..5).
        Step::Outfit if w.edit != Some(Edit::Shoes) => {
            for (k, &row) in side.iter().enumerate() {
                let lit = k == usize::from(b.clothing_variant);
                v.rows.push(RowWords {
                    row,
                    template: t1,
                    words: look
                        .text("editplyr_clothes.txt", 3 + k)
                        .unwrap_or("Color")
                        .to_owned(),
                    lit,
                });
                // The cursor's frame on the colour worn (78 by 33 about the
                // row, 0xa5860b as `my_team_wizard_outfit.png` has it).
                if lit && let Some(at) = element(layout, 10, row).and_then(spot) {
                    let r = Rect::from_center_size(at + Vec2::new(0.5, 7.0), Vec2::new(78.0, 33.0));
                    outline(&mut v.boxes, r, 2.0, [0xa5, 0x86, 0x0b, 0xff]);
                }
            }
        }
        Step::First | Step::Last | Step::Icon => {
            name_panel(look, layout, w, &mut v, portraits, images)
        }
        _ => {}
    }
    v
}

/// A frame `width` thick inside `r`.
fn outline(boxes: &mut Vec<(Rect, [u8; 4])>, r: Rect, width: f32, colour: [u8; 4]) {
    let (min, max) = (r.min, r.max);
    boxes.push((
        Rect::from_corners(min, Vec2::new(max.x, min.y + width)),
        colour,
    ));
    boxes.push((
        Rect::from_corners(Vec2::new(min.x, max.y - width), max),
        colour,
    ));
    boxes.push((
        Rect::from_corners(min, Vec2::new(min.x + width, max.y)),
        colour,
    ));
    boxes.push((
        Rect::from_corners(Vec2::new(max.x - width, min.y), max),
        colour,
    ));
}

/// The skills panel (`FUN_0027d360`, `FUN_0027d5f0`): each skill's name in
/// column 0 (the template 160), its crowns in column 1 (a kind-20 meter at
/// the row), its next step's price in column 2 (the template 220,
/// `createplyr_skills.txt` 4 "%1d DP", 5 "Max").
fn skills_rows(look: &Look, layout: &Layout, w: &Wizard, v: &mut Values, (t0, t1): (u32, u32)) {
    let mut cols: Vec<(u32, u32, u32)> = layout
        .elements
        .iter()
        .filter(|e| e.kind == 10 && e.props.get(&33) == Some(&20))
        .map(|e| {
            (
                e.props.get(&34).copied().unwrap_or(0),
                e.id,
                e.position().map_or(0, |p| u32::from(p.0)),
            )
        })
        .collect();
    cols.sort_unstable();
    let names = list(look, w.baller, Step::Skills, w.extras).0;
    for (k, row) in cols.chunks(3).enumerate().take(7) {
        let lit = k == w.skill;
        let [name, meter, price] = [
            row[0].1,
            row.get(1).map_or(0, |r| r.1),
            row.get(2).map_or(0, |r| r.1),
        ];
        if let Some(words) = names.get(k) {
            v.rows.push(RowWords {
                row: name,
                template: t0,
                words: words.clone(),
                lit,
            });
        }
        let level = crate::ballers::skill_level(k, w.baller.skills[k]);
        v.row_meters.push((meter, level.min(6) as u8));
        // The focused skill's arrow past its crowns while it can rise (at
        // the meter's end, as `my_team_wizard_skills.png` shows it).
        if lit && level < 6 {
            v.arrows.push((meter, Vec2::new(63.0, 6.0)));
        }
        let words = match w.skill_prices[k] {
            Some(n) => dp_words(look, "createplyr_skills.txt", 4, "%1d DP", i64::from(n)),
            None => look
                .text("createplyr_skills.txt", 5)
                .unwrap_or("Max")
                .to_owned(),
        };
        v.rows.push(RowWords {
            row: price,
            template: t1,
            words,
            lit,
        });
    }
    v.hidden.extend([4, 5]);
}

/// The name panel (`FUN_0027c160`): "Enter First Name", "Enter Last Name"
/// (`createplyr_name.txt` 6, 7) over the keyboard at element 11
/// (`FUN_0023c7b8` with RANDOM), or "Select Baller Icon" (8) over the
/// icons in list 0x2c (portrait sheets 0xb0..0xb6, `FUN_00276230` with
/// `0x3f8b80`, seven), the title "Choose Icon" (9).
fn name_panel(
    look: &Look,
    layout: &Layout,
    w: &Wizard,
    v: &mut Values,
    portraits: Option<&mut crate::portraits::Portraits>,
    images: &mut Assets<Image>,
) {
    let line = |k: usize, or: &str| look.text("createplyr_name.txt", k).unwrap_or(or).to_owned();
    match w.step {
        Step::Icon => {
            v.words.push((2, line(9, "Choose Icon")));
            v.words.push((10, line(8, "Select Baller Icon")));
            let side = list_rows(layout, 44);
            let icon = usize::from(w.baller.icon.min(6));
            let top = scrolled(w.icon_top, icon, 7, side.len());
            if top == 0 {
                v.hidden.push(6);
            }
            if top + side.len() >= 7 {
                v.hidden.push(7);
            }
            // The icons: portrait tiles (`FUN_00223698`: a created
            // player's back, palette 11, the sheet's picture, the frame)
            // on the rows' points, the list dimming all but the focused one
            // to 0x9f (`FUN_00222420`).
            if let (Some(portraits), Some(pm)) = (portraits, look.prematch.as_ref()) {
                for (k, &row) in side.iter().enumerate() {
                    let Some(at) = element(layout, 10, row).and_then(spot) else {
                        continue;
                    };
                    let bright = if top + k == icon {
                        1.0
                    } else {
                        0x9f as f32 / 255.0
                    };
                    if let Some(back) = pm.tile_back.get(11).or(pm.tile_back.first()) {
                        v.tiles.push((
                            back.clone(),
                            Rect::from_center_size(at, pm.tile_back_size),
                            bright,
                        ));
                    }
                    if let Some(image) = portraits.get(images, crate::portraits::ICONS + top + k) {
                        v.tiles.push((
                            image,
                            Rect::from_center_size(at, pm.tile_back_size),
                            bright,
                        ));
                    }
                    v.tiles.push((
                        pm.tile_frame.clone(),
                        Rect::from_center_size(at, pm.tile_size),
                        bright,
                    ));
                }
            }
            v.hidden.extend([11]);
        }
        Step::First | Step::Last => {
            v.words.push((
                10,
                if w.step == Step::First {
                    line(6, "Enter First Name")
                } else {
                    line(7, "Enter Last Name")
                },
            ));
            v.hidden.extend([6, 7]);
        }
        _ => {}
    }
}

/// The name panel's keyboard (element 11's point) with RANDOM under DONE
/// (`entername.txt` 8).
pub fn keyboard(
    out: &mut Vec<Piece>,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    look: &Look,
    layout: &Layout,
    typed: &str,
    key: (usize, usize),
) {
    let Some(at) = element(layout, 3, 11).and_then(spot) else {
        return;
    };
    crate::users::keyboard(out, labels, images, look, at, typed, key);
    let lit = key.0 >= crate::typing::NAME_KEYBOARD.len();
    let words = look.text("entername.txt", 8).unwrap_or("RANDOM");
    let colour = if lit { colours::HELP } else { colours::ROW };
    let line_h = look
        .fonts
        .get(fonts::SMALL)
        .map_or(18.0, |f| f.line_height as f32);
    let middle = at + Vec2::new(19.0 * 2.5 + 10.0, 144.0);
    let from = out.len();
    place(
        out,
        labels,
        look,
        images,
        fonts::SMALL,
        words,
        colour,
        middle - Vec2::new(0.0, line_h / 2.0),
        1,
    );
    if lit {
        for p in &mut out[from..] {
            let half = p.rect.half_size() * 1.25;
            let centre = middle + (p.rect.center() - middle) * 1.25;
            p.rect = Rect::from_center_half_size(centre, half);
        }
    }
}

/// A panels' screen and its L1 and R1 marks (kind 37): the arrow and the
/// icon each side.
#[derive(Clone, Copy)]
pub struct Screen {
    pub class: u32,
    pub left_arrow: u32,
    pub left_icon: u32,
    pub right_icon: u32,
    pub right_arrow: u32,
}

/// Create Baller's screen, layout 61.
pub const CREATE: Screen = Screen {
    class: 61,
    left_arrow: 158,
    left_icon: 157,
    right_icon: 155,
    right_arrow: 156,
};

/// Edit Baller's screen (`FUN_0028b768`: class 0x4f), layout 79.
pub const EDIT: Screen = Screen {
    class: 79,
    left_arrow: 150,
    left_icon: 17,
    right_icon: 18,
    right_arrow: 149,
};

/// A legend's first pair as a screen or panel sets it after the load
/// (`FUN_0021aa70(legend, 0, label)`, `FUN_0021aac0(legend, 0, action)`):
/// the legend (its element id: the editor screen's 16, Edit Moves' panel's
/// 15), the label's text id, the action whose icon it shows, how far its
/// icon is moved down (Edit Moves' step 1 raises Start's by 3,
/// `FUN_002921f8`), and whether the pairs past it are hidden
/// (`FUN_0021a9b8(legend, 1)`, The Moves).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LegendPair {
    pub element: u32,
    pub label: u32,
    pub action: u32,
    pub icon_dy: f32,
    pub alone: bool,
}

/// Edit Baller's and Edit Moves' "Exit" on Start (`my_team_editor.png`,
/// `my_team_edit_moves.png`; Edit Moves' step 1 sets it, `FUN_002921f8`).
pub const EXIT: LegendPair = LegendPair {
    element: 16,
    label: 0x13,
    action: 4,
    icon_dy: 0.0,
    alone: false,
};

/// The header: the premade's name by its top's middle on the list's node
/// (element 21, font 2, 0xfff6f4d7) between the L1 and R1 icons and their arrows
/// (layout 61's kind 37 marks: 158 and 156 the arrows, 157 and 155 the
/// icons); the wizard's panels name themselves (element 2) and show the
/// marks their step has.
#[allow(clippy::too_many_arguments)]
pub fn header(
    out: &mut Vec<Piece>,
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    layout: &Layout,
    name: Option<&str>,
    marks: (bool, bool),
    screen: Screen,
    pairs: &[LegendPair],
) {
    if let Some(name) = name
        && let Some(e) = layout.elements.iter().find(|e| e.kind == 10 && e.id == 21)
        && let Some(at) = spot(e)
    {
        place(
            out,
            labels,
            look,
            images,
            fonts::LIST,
            name,
            colours::PALE,
            at,
            1,
        );
    }
    let Screen {
        class,
        left_arrow,
        left_icon,
        right_icon,
        right_arrow,
    } = screen;
    let Some(screen) = look.layouts.iter().find(|l| l.class == class) else {
        return;
    };
    // The legends: the screen's Select and Cancel (layout 61), the
    // panel's Help; `createplyr.txt` is file 0x20, past `Look::text_id`'s.
    let text = |id: u16| look.text_full(id).map(str::to_owned);
    for e in screen
        .elements
        .iter()
        .chain(&layout.elements)
        .filter(|e| e.kind == 34)
    {
        // A legend's first pair (props 56 and 60) as the screen or the
        // panel set it.
        if let Some(pair) = pairs.iter().find(|p| p.element == e.id) {
            let mut e = e.clone();
            e.props.insert(56, pair.label);
            e.props.insert(60, pair.action);
            if pair.alone {
                for k in 57..60 {
                    e.props.remove(&k);
                }
            }
            let from = out.len();
            look::legend(out, labels, look, images, &e, &text);
            // The first pair is laid out last: its icon, then its label.
            let icon = out.len().saturating_sub(2).max(from);
            if pair.icon_dy != 0.0
                && let Some(p) = out.get_mut(icon)
            {
                p.rect.min.y += pair.icon_dy;
                p.rect.max.y += pair.icon_dy;
            }
        } else {
            look::legend(out, labels, look, images, e, &text);
        }
    }
    let mark = |id: u32| {
        screen
            .elements
            .iter()
            .find(|e| e.kind == 37 && e.id == id)
            .and_then(spot)
    };
    let (left, right) = marks;
    if let Some(pm) = look.prematch.as_ref() {
        for (id, image, shown) in [
            (left_arrow, &pm.arrow_left, left),
            (right_arrow, &pm.arrow_right, right),
        ] {
            if shown && let Some(at) = mark(id) {
                out.push(Piece::new(
                    image.clone(),
                    Rect::from_center_size(at, pm.arrow_side_size),
                ));
            }
        }
    }
    if let Some((atlas, frames)) = look.icons.as_ref() {
        for (id, frame, shown) in [(left_icon, 5, left), (right_icon, 6, right)] {
            if shown && let (Some(at), Some(&f)) = (mark(id), frames.get(frame)) {
                out.push(Piece::part(
                    atlas.clone(),
                    f,
                    Rect::from_center_size(at, f.size()),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_picture_sits_by_its_anchor() {
        let r = anchored(Vec2::new(100.0, 50.0), Vec2::new(20.0, 10.0), 1 | 4);
        assert_eq!(r, Rect::new(90.0, 45.0, 110.0, 55.0));
        let r = anchored(Vec2::new(100.0, 50.0), Vec2::new(20.0, 10.0), 0);
        assert_eq!(r.min, Vec2::new(100.0, 50.0));
    }

    #[test]
    fn the_tab_icons_follow_the_table() {
        assert_eq!(TAB_FRAMES[0], 8);
        assert_eq!(TAB_FRAMES.len(), 8);
        // Each panel lights its own tab with the image's lit row (10..19).
        for step in STEPS {
            let (tab, frame) = step.tab();
            assert!((90..=96).contains(&tab));
            assert!((10..=19).contains(&frame));
        }
    }

    #[test]
    fn the_steps_go_the_original_way_round() {
        // A man: Build, Look, Hair, Facial Hair, Outfit, Skills, Name.
        let mut at = Step::Build;
        let mut man = vec![at];
        while let Some(next) = at.next(false) {
            man.push(next);
            at = next;
        }
        assert_eq!(man, STEPS.to_vec());
        // A woman skips Facial Hair both ways (`FUN_0027a8f0`, `FUN_00278a48`).
        assert_eq!(Step::Hair.next(true), Some(Step::Outfit));
        assert_eq!(Step::Outfit.back(true), Some(Step::Hair));
        assert_eq!(Step::Outfit.back(false), Some(Step::Facial));
        assert_eq!(Step::Build.back(false), None);
        assert_eq!(Step::Icon.next(false), None);
        for (k, step) in STEPS.into_iter().enumerate() {
            assert_eq!(Step::at(k), step);
            assert_eq!(step.index(), k);
        }
    }

    #[test]
    fn the_marks_follow_the_panels() {
        assert_eq!(Step::Build.marks(), (false, true));
        assert_eq!(Step::Skills.marks(), (true, true));
        assert_eq!(Step::Icon.marks(), (true, false));
    }

    #[test]
    fn the_framing_closes_on_the_head_for_the_look_panels() {
        assert_eq!(Step::Look.framing(), Framing::Head);
        assert_eq!(Step::Facial.framing(), Framing::Head);
        assert_eq!(Step::Build.framing(), Framing::Body);
        assert_eq!(Step::Icon.framing(), Framing::Body);
    }

    #[test]
    fn the_editor_goes_round_its_panels() {
        assert_eq!(Edit::Outfit.turn(1, false), Edit::Shoes);
        assert_eq!(Edit::Outfit.turn(-1, false), Edit::Skills);
        assert_eq!(Edit::Hair.turn(1, true), Edit::Skills);
        assert_eq!(Edit::Skills.turn(-1, true), Edit::Hair);
        assert_eq!(Edit::Facial.layout(), 83);
        assert_eq!(Edit::Hair.framing(), Framing::Head);
        for (k, e) in EDITS.into_iter().enumerate() {
            assert_eq!(Edit::at(k), e);
            assert!((10..=19).contains(&e.tab().1));
        }
    }

    #[test]
    fn the_questions_open_on_no_or_their_one_row() {
        assert_eq!(CabAsk::Exit.opening_row(), 1);
        assert_eq!(CabAsk::Leave { back_up: true }.opening_row(), 1);
        assert_eq!(
            CabAsk::Buy {
                cost: 50,
                name: "Outfit B".into()
            }
            .opening_row(),
            1
        );
        assert_eq!(CabAsk::WizardPoor.opening_row(), 0);
        assert_eq!(
            CabAsk::Poor {
                name: String::new()
            }
            .opening_row(),
            0
        );
    }

    #[test]
    fn a_long_list_scrolls_to_keep_the_cursor() {
        assert_eq!(scrolled(0, 3, 4, 4), 0);
        assert_eq!(scrolled(0, 6, 18, 7), 0);
        assert_eq!(scrolled(0, 7, 18, 7), 1);
        assert_eq!(scrolled(5, 2, 18, 7), 2);
        assert_eq!(scrolled(20, 17, 18, 7), 11);
    }
}
