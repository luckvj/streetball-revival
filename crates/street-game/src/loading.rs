//! The loading screen's picture (`/FE/XAFELOAD.BIN`, a BIG archive of 47
//! sheets, each ten 128x256 strips making a 640x512 picture).
//!
//! What the game shows going into a game (`FUN_0029c3a8`, by the front
//! end's mode `rec + 0x78`): the 25 legend cards (0..24) in turn, each
//! once before any repeats (`FUN_0029a828`), for Street School (mode 1),
//! the League Challenge (4) and Be A Legend (5); the game's logo (34,
//! `FUN_0029a668`) for the attract's game (0); and for Pick Up (2, 3) a
//! card and one of four street tips in turn (`FUN_0029a8c0`: pictures
//! 26..29, whose words `crate::loading_view` draws, `FUN_0029b828`). The
//! cards' object (`FUN_0029a510`) is made at the boot (class 4,
//! `FUN_0029dca8`) and marks the first `draw % 25` cards shown, so the
//! first card is a random one. Card 25 is a Michael Jordan card the French
//! game shows in place of card 3.
//!
//! The load back to the front end after a game (class 0x2f, `FUN_0029d420`)
//! picks from the same object ([`LoadingCards::back`]): Be A Legend's
//! "STREET" magazine covers (35..46) by the career's progress
//! (`FUN_0029a670`); for Pick Up the Street Tips pages of the tip cards
//! owned (`FUN_0029a5c0`, `FUN_0029a980`: two in a row, `XAFEBG` backdrop
//! 0x1a with `streetTips.txt`'s words) and between them one of tips 4..7
//! (pictures 30..33); the logo for everything else.

use bevy::{
    asset::RenderAssetUsages,
    prelude::*,
    render::render_resource::{Extent3d, TextureDimension, TextureFormat},
};
use street_assets::{big, ssh};

use crate::setup::Disc;

const ARCHIVE: &str = "/FE/XAFELOAD.BIN";
const LEGENDS: u32 = 25;
const LOGO: usize = 34;
/// The street tips' pictures (`FUN_0029b828`: the tip's index plus 0x1a).
const TIPS: usize = 0x1a;
/// Pick Up's tips going into a game (`+0x8`'s low four bits).
const PICK_UP_TIPS: u8 = 4;
/// The tips the load back shows (`FUN_0029a980` state 1: `+0x8`'s high
/// four bits).
const BACK_TIPS: std::ops::Range<u8> = 4..8;
/// Be A Legend's first cover (`FUN_0029a670`: 0x23 + the cover) and how
/// many there are.
const COVERS: usize = 0x23;
const COVER_COUNT: u16 = 12;
/// The Street Tips cards (`XDBCARD` category 5, `FUN_001d3200(.., k)` for k
/// below 0x14).
pub const STREET_TIP_CARDS: u32 = 20;
const STREET_TIP_MASK: u32 = (1 << STREET_TIP_CARDS) - 1;
/// The Street Tips page's picture: `FUN_0029b720` returns 0x1a with
/// `+0x140` 0, which `FUN_00235758` reads as an `XAFEBG` backdrop, not a
/// loading picture.
pub const STREET_TIPS_BACKDROP: usize = 0x1a;

/// Which pictures have shown this session (`DAT_003fa4e0`).
#[derive(Resource)]
pub struct LoadingCards {
    /// Legend cards shown, one bit each (`+0xc`); all shown starts over.
    shown: u32,
    /// Pick Up's tips shown, one bit each (`+0x8`): 0..3 going into a game,
    /// 4..7 on the way back.
    tips: u8,
    /// Pick Up's next load shows a tip (`+0x13` 1), else a card (2, as
    /// made).
    tip_next: bool,
    /// Street Tips cards shown on the way back (`+0x0`, twenty bits).
    street_shown: u32,
    /// Street Tips cards owned (`+0x4`): `FUN_0029a5c0` sets each owned
    /// card's bit before every pick and never clears one.
    street_owned: u32,
    /// The load back's next page is a tip (`+0x12` 1), else a Street Tips
    /// card (0, as made).
    back_tip_next: bool,
    /// The second of a pair of Street Tips pages comes next (`+0x14`).
    pair: bool,
    /// Covers shown since the career's end, one bit each (`+0x10`).
    covers: u16,
    /// The cover the career's progress last showed (`+0x1b`, 0xff none).
    last_cover: u8,
}

impl Default for LoadingCards {
    fn default() -> Self {
        Self {
            shown: 0,
            tips: 0,
            tip_next: false,
            street_shown: 0,
            street_owned: 0,
            back_tip_next: false,
            pair: false,
            covers: 0,
            last_cover: 0xff,
        }
    }
}

/// The front end's mode (`rec + 0x78`) of the game on screen, which both
/// loads' pictures read; set as the game loads.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Mode {
    /// The attract's game (0).
    #[default]
    Attract,
    /// Street School (1).
    School,
    /// A Pick Up game (2, 3), the rewrite's quick and online games too.
    PickUp,
    /// The League Challenge (4).
    League,
    /// Be A Legend (5).
    Legend,
}

impl Mode {
    /// The to-game load's choice ([`LoadingCards::next`]).
    pub fn kind(self) -> Kind {
        match self {
            Mode::Attract => Kind::Attract,
            Mode::PickUp => Kind::PickUp,
            Mode::School | Mode::League | Mode::Legend => Kind::Card,
        }
    }
}

/// What a load is for, as the picture's choice reads the front end's
/// mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// The attract's game (mode 0).
    Attract,
    /// Street School (1), the League Challenge (4) and Be A Legend (5).
    Card,
    /// A Pick Up game (2, 3), the rewrite's quick and online games too.
    PickUp,
}

/// What the load back to the front end shows (`FUN_0029d420`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Back {
    /// A loading picture: the logo or a cover.
    Picture(usize),
    /// A street tip's page, 4..7 (`FUN_0029b828`: picture 0x1a + the tip
    /// with its words).
    Tip(u8),
    /// The Street Tips page of the owned tip card 0..19 (`FUN_0029b720`).
    StreetTip(u8),
}

impl Back {
    /// The loading picture it shows (a Street Tips page shows a backdrop).
    pub fn picture(self) -> Option<usize> {
        match self {
            Back::Picture(k) => Some(k),
            Back::Tip(tip) => Some(TIPS + usize::from(tip)),
            Back::StreetTip(_) => None,
        }
    }
}

/// The career's standing the covers read (`FUN_0029a670`, the career at
/// game data `+0xf10`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Career {
    /// Overall progress points (`FUN_001e73d8`).
    pub progress: u32,
    /// The cover a won tournament earns: 5 once event 3 is cleared, else
    /// 4 once event 2 is (`FUN_001e7ff8(.., 3)`, `(.., 2)`).
    pub special: Option<u8>,
    /// Rucker Park '78's challenge, the last, is won (`FUN_001e8140`).
    pub done: bool,
}

impl Career {
    pub fn of(story: &street_sim::story::Story) -> Self {
        let special = if story.cleared(3) {
            Some(5)
        } else if story.cleared(2) {
            Some(4)
        } else {
            None
        };
        // The first event on court 10 (`FUN_001eab50(ev) == 10`).
        let done = street_sim::story::EVENTS
            .iter()
            .position(|e| e.court == 10)
            .is_some_and(|ev| story.cleared(ev));
        Self {
            progress: story.overall_points(),
            special,
            done,
        }
    }
}

/// The Street Tips cards owned, one bit per card's index
/// (`FUN_001d26e8` through `FUN_001d3200(.., k)`).
pub fn owned_street_tips(
    cards: &[street_assets::db::Card],
    story: &street_sim::story::Story,
) -> u32 {
    cards
        .iter()
        .filter(|c| c.category == crate::rewards::TIPS && crate::rewards::card_owned(story, c))
        .filter_map(|c| u32::try_from(c.index).ok())
        .filter(|&k| k < STREET_TIP_CARDS)
        .fold(0, |m, k| m | 1 << k)
}

impl LoadingCards {
    /// `FUN_0029a510`: the first `draw % 25` cards marked shown, `draw`
    /// the shared generator's (`FUN_003a16b8`) at the boot.
    pub fn new(draw: u32) -> Self {
        let start = draw % LEGENDS;
        Self {
            shown: (1 << start) - 1,
            ..Default::default()
        }
    }

    /// The next picture for a load, and the street tip it shows when it is
    /// one.
    pub fn next(&mut self, kind: Kind) -> (usize, Option<u8>) {
        match kind {
            Kind::Attract => (LOGO, None),
            Kind::Card => (self.card(), None),
            Kind::PickUp if self.tip_next => {
                // `FUN_0029a8c0` state 1: the first tip not shown; all
                // shown, the round starts over.
                self.tip_next = false;
                let tip = (0..PICK_UP_TIPS)
                    .find(|&k| self.tips & (1 << k) == 0)
                    .unwrap_or_else(|| {
                        self.tips &= 0xf0;
                        0
                    });
                self.tips |= 1 << tip;
                (TIPS + usize::from(tip), Some(tip))
            }
            Kind::PickUp => {
                self.tip_next = true;
                (self.card(), None)
            }
        }
    }

    /// The load back's picture (`FUN_0029d420`) for a game of `mode`: Pick
    /// Up's Street Tips pages and tips 4..7, Be A Legend's cover by
    /// `career`, else the logo (`FUN_0029a668`). `owned` is the Street
    /// Tips cards owned ([`owned_street_tips`]).
    pub fn back(&mut self, mode: Mode, owned: u32, career: Career) -> Back {
        match mode {
            Mode::PickUp => {
                self.street_owned |= owned & STREET_TIP_MASK;
                self.pick_up_back()
            }
            Mode::Legend => Back::Picture(self.cover(career)),
            _ => Back::Picture(LOGO),
        }
    }

    /// `FUN_0029a980`: state 0 shows the first owned card not shown yet, a
    /// pair of them before a tip (`+0x14` turns over each card); with none
    /// owned, or every one shown (which starts the round over), state 1
    /// shows the first of tips 4..7 not shown (all shown: the round starts
    /// over at 4).
    fn pick_up_back(&mut self) -> Back {
        loop {
            if !self.back_tip_next {
                let owned = self.street_owned & STREET_TIP_MASK;
                if owned != 0 && self.street_shown & STREET_TIP_MASK != owned {
                    let k = (0..STREET_TIP_CARDS)
                        .find(|&k| owned & (1 << k) != 0 && self.street_shown & (1 << k) == 0);
                    if let Some(k) = k {
                        self.back_tip_next = self.pair;
                        self.pair = !self.pair;
                        self.street_shown |= 1 << k;
                        return Back::StreetTip(k as u8);
                    }
                }
                self.street_shown = 0;
                self.back_tip_next = true;
                continue;
            }
            match BACK_TIPS.clone().find(|&t| self.tips & (1 << t) == 0) {
                Some(tip) => {
                    self.tips |= 1 << tip;
                    self.back_tip_next = false;
                    return Back::Tip(tip);
                }
                None => self.tips &= 0x0f,
            }
        }
    }

    /// `FUN_0029a670`: before the career's end (progress under 500 and
    /// Rucker Park '78 not won) the cover its progress reaches (0, 1, 2, 3
    /// from 0, 9, 17, 45 points; 6..11 from 130, 160, 200, 300, 400, 450),
    /// and once a tournament's cover is earned it stands in for the
    /// progress's when that one showed last (or for the first four);
    /// after the end, every cover in turn.
    fn cover(&mut self, career: Career) -> usize {
        if career.progress < 500 && !career.done {
            let by = match career.progress {
                0..=8 => 0,
                9..=16 => 1,
                17..=44 => 2,
                45..=129 => 3,
                130..=159 => 6,
                160..=199 => 7,
                200..=299 => 8,
                300..=399 => 9,
                400..=449 => 10,
                _ => 11,
            };
            let shown = match career.special {
                None => {
                    self.last_cover = by;
                    by
                }
                Some(special) if self.last_cover == 0xff => {
                    if by > 3 {
                        self.last_cover = by;
                        by
                    } else {
                        special
                    }
                }
                Some(special) if self.last_cover == by => special,
                Some(_) => by,
            };
            return COVERS + usize::from(shown);
        }
        match (0..COVER_COUNT).find(|&k| self.covers & (1 << k) == 0) {
            Some(k) => {
                self.covers |= 1 << k;
                COVERS + usize::from(k)
            }
            None => {
                self.covers = 1;
                COVERS
            }
        }
    }

    /// `FUN_0029a828`: the first card not shown; all shown, card 0 and the
    /// round starts over from it.
    fn card(&mut self) -> usize {
        match (0..LEGENDS).find(|&k| self.shown & (1 << k) == 0) {
            Some(k) => {
                self.shown |= 1 << k;
                k as usize
            }
            None => {
                self.shown = 1;
                0
            }
        }
    }
}

/// Reads picture `k` and puts its strips together (the top 640x480 shows),
/// or the HD pack's picture when one is set and has it.
pub fn picture(disc: &mut Disc, images: &mut Assets<Image>, k: usize) -> Option<Handle<Image>> {
    let index = big::read_index(&mut disc.iso, ARCHIVE).ok()?;
    let entry = index.get(k)?;
    let bytes = big::read_entry(&mut disc.iso, ARCHIVE, entry).ok()?;
    let sheet = ssh::parse(&bytes).ok()?;
    let (w, h) = (640usize, 512usize);
    let mut rgba = vec![0u8; w * h * 4];
    for (i, t) in sheet.textures.iter().take(10).enumerate() {
        let (x0, y0) = ((i % 5) * 128, (i / 5) * 256);
        for y in 0..t.height as usize {
            let row = y0 + y;
            if row >= h {
                break;
            }
            for x in 0..t.width as usize {
                let col = x0 + x;
                if col >= w {
                    break;
                }
                let s = (y * t.width as usize + x) * 4;
                let d = (row * w + col) * 4;
                rgba[d..d + 4].copy_from_slice(&t.rgba[s..s + 4]);
            }
        }
    }
    let whole = ssh::Texture {
        name: String::new(),
        width: w as u32,
        height: h as u32,
        rgba,
        indices: None,
    };
    let shown = street_render::hires::picture(k, &whole).unwrap_or(whole);
    Some(images.add(top(shown)))
}

/// The top 15/16 of a whole 640x512 picture (the 640x480 that shows) at
/// whatever resolution it comes in.
fn top(mut t: ssh::Texture) -> Image {
    let height = t.height * 15 / 16;
    t.rgba.truncate((t.width * height * 4) as usize);
    Image::new(
        Extent3d {
            width: t.width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        t.rgba,
        TextureFormat::Rgba8UnormSrgb,
        RenderAssetUsages::RENDER_WORLD,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legends_go_round_and_the_attract_shows_the_logo() {
        let mut c = LoadingCards::default();
        let first: Vec<usize> = (0..25).map(|_| c.next(Kind::Card).0).collect();
        assert_eq!(first, (0..25).collect::<Vec<_>>());
        assert_eq!(c.next(Kind::Card).0, 0); // all shown: round again
        assert_eq!(c.next(Kind::Card).0, 1);
        assert_eq!(c.next(Kind::Attract), (LOGO, None));
        assert_eq!(c.next(Kind::Card).0, 2);
    }

    #[test]
    fn the_first_card_is_the_boots_draw() {
        // `FUN_0029a510`: draw 57 marks 57 % 25 = 7 cards shown.
        let mut c = LoadingCards::new(57);
        assert_eq!(c.next(Kind::Card).0, 7);
        assert_eq!(c.next(Kind::Card).0, 8);
        let mut c = LoadingCards::new(50);
        assert_eq!(c.next(Kind::Card).0, 0);
        // The round wraps to 0 once 7..24 have shown, then 1.
        let mut c = LoadingCards::new(24);
        assert_eq!(c.next(Kind::Card).0, 24);
        assert_eq!(c.next(Kind::Card).0, 0);
        assert_eq!(c.next(Kind::Card).0, 1);
    }

    #[test]
    fn pick_up_takes_a_card_then_a_tip() {
        let mut c = LoadingCards::default();
        let seen: Vec<_> = (0..10).map(|_| c.next(Kind::PickUp)).collect();
        assert_eq!(
            seen,
            vec![
                (0, None),
                (26, Some(0)),
                (1, None),
                (27, Some(1)),
                (2, None),
                (28, Some(2)),
                (3, None),
                (29, Some(3)),
                (4, None),
                (26, Some(0)),
            ]
        );
        // Other loads take cards from the one round.
        assert_eq!(c.next(Kind::Card).0, 5);
        assert_eq!(c.next(Kind::PickUp), (6, None));
    }

    fn backs(c: &mut LoadingCards, owned: u32, n: usize) -> Vec<Back> {
        (0..n)
            .map(|_| c.back(Mode::PickUp, owned, Career::default()))
            .collect()
    }

    #[test]
    fn pick_up_comes_back_on_tips_four_to_seven_with_no_tip_cards() {
        // Measured: the runtime's Pick Up games (no cards owned) came back
        // on picture 30, "Be the Oop" (tip 4).
        let mut c = LoadingCards::default();
        use Back::Tip;
        assert_eq!(
            backs(&mut c, 0, 5),
            vec![Tip(4), Tip(5), Tip(6), Tip(7), Tip(4)]
        );
        // The to-game tips keep their own four bits.
        assert_eq!(c.next(Kind::PickUp), (0, None));
        assert_eq!(c.next(Kind::PickUp), (26, Some(0)));
        assert_eq!(backs(&mut c, 0, 1), vec![Tip(5)]);
        assert_eq!(Tip(5).picture(), Some(31));
        assert_eq!(Back::StreetTip(3).picture(), None);
    }

    #[test]
    fn owned_street_tips_come_in_pairs_between_tips() {
        use Back::{StreetTip as S, Tip as T};
        // One card owned: shown, the round over (a tip); shown again, and
        // the pair's flag sends a tip after it; then the round over again.
        let mut c = LoadingCards::default();
        assert_eq!(
            backs(&mut c, 1 << 3, 8),
            vec![S(3), T(4), S(3), T(5), T(6), S(3), T(7), S(3)]
        );
        // Three: two cards, a tip; the third, the round over (a tip), and
        // on from the first.
        let mut c = LoadingCards::default();
        assert_eq!(
            backs(&mut c, 0b100110, 10),
            vec![S(1), S(2), T(4), S(5), T(5), S(1), T(6), S(2), S(5), T(7)]
        );
        // A card owned later joins the round (`FUN_0029a5c0` only adds).
        let mut c = LoadingCards::default();
        assert_eq!(backs(&mut c, 0, 1), vec![T(4)]);
        assert_eq!(backs(&mut c, 1 << 7, 2), vec![S(7), T(5)]);
        assert_eq!(backs(&mut c, 0, 1), vec![S(7)]);
    }

    #[test]
    fn be_a_legend_comes_back_on_its_covers_by_progress() {
        let mut c = LoadingCards::default();
        let at = |progress| Career {
            progress,
            ..Default::default()
        };
        // Measured: the first won story game came back on picture 35.
        for (progress, cover) in [
            (0, 0),
            (8, 0),
            (9, 1),
            (16, 1),
            (17, 2),
            (44, 2),
            (45, 3),
            (129, 3),
            (130, 6),
            (160, 7),
            (200, 8),
            (300, 9),
            (400, 10),
            (450, 11),
            (499, 11),
        ] {
            assert_eq!(
                c.back(Mode::Legend, 0, at(progress)),
                Back::Picture(35 + cover)
            );
        }
        // A won tournament's cover stands in for the progress's first four.
        let mut c = LoadingCards::default();
        let won = |progress, special| Career {
            progress,
            special: Some(special),
            done: false,
        };
        assert_eq!(c.back(Mode::Legend, 0, won(20, 4)), Back::Picture(39));
        assert_eq!(c.back(Mode::Legend, 0, won(20, 5)), Back::Picture(40));
        // From cover 6 the progress's shows once and is remembered; then
        // the tournament's while the progress stays on it.
        assert_eq!(c.back(Mode::Legend, 0, won(140, 5)), Back::Picture(41));
        assert_eq!(c.back(Mode::Legend, 0, won(140, 5)), Back::Picture(40));
        assert_eq!(c.back(Mode::Legend, 0, won(170, 5)), Back::Picture(42));
        // The career done: every cover in turn, then over.
        let mut c = LoadingCards::default();
        let done = Career {
            progress: 500,
            special: Some(5),
            done: true,
        };
        let turn: Vec<_> = (0..13).map(|_| c.back(Mode::Legend, 0, done)).collect();
        let want: Vec<_> = (35..47).chain([35]).map(Back::Picture).collect();
        assert_eq!(turn, want);
        assert_eq!(c.back(Mode::Legend, 0, done), Back::Picture(36));
        // Under 500 with '78 won counts as done too.
        let mut c = LoadingCards::default();
        let won78 = Career {
            progress: 40,
            special: None,
            done: true,
        };
        assert_eq!(c.back(Mode::Legend, 0, won78), Back::Picture(35));
    }

    #[test]
    fn the_rest_come_back_on_the_logo() {
        let mut c = LoadingCards::default();
        for mode in [Mode::Attract, Mode::School, Mode::League] {
            assert_eq!(
                c.back(mode, 0xfffff, Career::default()),
                Back::Picture(LOGO)
            );
        }
        assert_eq!(Mode::Legend.kind(), Kind::Card);
        assert_eq!(Mode::PickUp.kind(), Kind::PickUp);
    }
}
