//! The front end's screen transitions: what the original plays between one
//! of its screens and the next (the front end's slot `+0x5c`,
//! `FUN_002ddb40`), as measured in the project's PCSX2 runtime (PINE on the
//! front end `*0x3fe910`: its screen `+0x3c`, request `+0x40` and
//! transition `+0x4c`; the screen's state `+0x6c`; the 100 Hz timer
//! `0x4b32e4`; PrintWindow frames every 42 ms).
//!
//! A change of screen runs in four steps:
//!
//! 1. *Leaving.* The press puts the old screen in state 1 (`FUN_002d7cb0`,
//!    forward, the confirm sound) or 2 (`FUN_002d7d40`, back, the cancel
//!    sound). Its widgets play their records for that state
//!    (`FUN_00249cb0`: on Main, Play It and the other screens over the main
//!    backdrop the title slides off to the left, the other widgets fade)
//!    until the screen's deadline for it (`FUN_002d7e18` against the times
//!    `FUN_002d8398` sets: 266.67 ms there, 0 elsewhere), when it asks the
//!    front end for the change. Presses meanwhile reach the old screen,
//!    which drops them.
//! 2. *Holding.* The front end builds the new screen (`FUN_002dcff8`),
//!    hides it and starts the movie its slot `+0x154` names (a forward
//!    change) or `xmtblur.nfx` when its slot `+0x174` allows (a back one),
//!    full screen at (0, -16) of the 448-line frame (`+0x11c`): the whole
//!    640x480 of the layouts. The movie's first frame shows about 0.2 s
//!    later; the frame meanwhile keeps the old screen's last picture.
//! 3. *Playing.* The movie runs at 29.97 frames a second. The front end
//!    updates no screen while it plays (`FUN_002dd338` returns while
//!    `+0x4c` is set), so presses wait in its queue.
//! 4. *Arriving.* The movie over, the new screen shows in state 0: its
//!    widgets play their arriving records (they fade in; on the main
//!    backdrop's screens the title slides in from the left) until its
//!    deadline for state 0, then state 4, when the presses that waited
//!    reach it (a press during a movie moved Options' cursor the moment
//!    Options stood).
//!
//! The movie's start also plays the front end's transition sound
//! (`FUN_001a19b8` on its name: `FE_EVT` event 2, [`whoosh`]).
//!
//! A change can play several movies one after another: the movie task's
//! queue (`FUN_0022dce8`, five at most), each opened as the one before
//! ends (`FUN_0022d478`, `FUN_0022d958`). The first title's transition
//! plays the boot's movies ([`BOOT_REEL`]), the title's later ones its
//! intro and the zoom ([`TITLE_REEL`]), the credits the NuFX logo and
//! `xmtfblur`, and a DVD page's its movies into the page itself
//! ([`Change::dvd`]). While a change's movies play the front end empties
//! its pad events every update (`0x2de7b8`, `FUN_002e6ed0`): a press then
//! reaches no screen. Before it does, the title's (at the boot only during
//! the intro, its index 2; later until a Start was taken, the front end's
//! `+0x70`) and the DVD pages' (always) look through them for Start
//! (`FUN_002e14f8`, event 0x3d): on the title it ends the movie and plays
//! the zoom alone, on a DVD page it ends the change.

use bevy::prelude::*;

use crate::{
    Screen,
    look::{self, Piece, Role},
    movie,
    setup::Disc,
};

/// A forward change (`FUN_002d7cb0`, the old screen's state 1) or a back
/// one (`FUN_002d7d40`, state 2).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Forward,
    Back,
}

/// The class a screen reports at `+0x40`, which the transition slots
/// compare the screen before against: its layout's, passed to the screens'
/// base `FUN_002d7720`. It differs from the factory's class
/// (`FUN_002e55c0`, which picks the screen's slots) where screens share a
/// constructor: User Setup (`FUN_00297848`), Choose Team
/// (`FUN_0026fb00`), the loading screens (`FUN_0029aac8`), Choose Baller
/// and Create Baller (`FUN_002cd930`, `FUN_00273a08`), Create Team and the
/// crew (`FUN_0027dd80`), the editor's screens (`FUN_0028b768`).
pub fn reported(class: u16) -> u16 {
    match class {
        0x0c..=0x13 => 0x0b,
        0x1b | 0x1c => 0x1a,
        0x2c | 0x2d | 0x2f => 0x2c,
        0x36 | 0x3d => 0x36,
        0x35 | 0x62 => 0x35,
        0x4a..=0x4c => 0x3d,
        0x4f..=0x55 | 0x65 => 0x4f,
        0x07 | 0x08 => 0x07,
        c => c,
    }
}

/// The movie a forward change into the screen of class `to` plays, the
/// screen before it reporting `from` (each class's slot `+0x154`, raw MIPS
/// where the decompile stops at `FUN_0030fa50`). `legend`: the front end
/// is in Be A Legend (`FUN_001e0bc0() +0x78` = 5). The base slot
/// (`FUN_002d8f68`) names `xmtfblur.nfx`; an empty name plays nothing.
pub fn forward_movie(to: u16, from: u16, legend: bool) -> Option<&'static str> {
    Some(match to {
        // `FUN_0029f108` (`xmtjp00.nfx` on a Japanese console).
        0x03 => match from {
            0x35 => "xmtfblur.nfx",
            0x4d => "xmt0013.nfx",
            _ => "xmt0000.nfx",
        },
        // `FUN_002bfac0`, `FUN_002a45a8`, `FUN_0028a670`, `FUN_0028b620`,
        // `FUN_00289730` (whose own name from themselves is the DVD's
        // movie), `FUN_00284f30`; Choose Court's and the boards' and Create
        // Baller's `FUN_002a4b80`, `FUN_00276bd8`, `FUN_00272fe0`.
        0x05 | 0x06 | 0x66..=0x69 | 0x3b | 0x4a..=0x4d => return None,
        // The title's own slot (`FUN_002d41d0`) names nothing, but
        // `FUN_002ddb40` names the title's movie itself once a title has
        // stood (the front end's `+0x6c`, set by the first title's
        // transition, `0x2de07c`): `movies/xmt0018.nfx`, the intro
        // (`xmtpl18.nfx` on a PAL console; `0x2e0c1c`). The first title of
        // a boot follows the boot's movies (the rewrite's intro screen)
        // through no transition; the title comes forward again only by the
        // idle demo's turn ([`crate::attract`]).
        0x63 => "xmt0018.nfx",
        // The credits (`FUN_00281a08`, reported class 7): `FUN_00283b40`
        // names the NuFX logo for the screen built as class 8 (`+0x134`),
        // nothing for class 7; [`Transition::reel`] adds `xmtfblur.nfx`
        // after either.
        0x08 => "xnufx.nfx",
        0x07 => return None,
        // `FUN_00298dd8`.
        0x0c..=0x13 => "xmt0001.nfx",
        // `FUN_002c5598`.
        0x14 => "xmtblur3.nfx",
        // `FUN_00272ef0`, `FUN_00272f98`.
        0x19 => "xmt0019.nfx",
        0x1b | 0x1c => "xmt0002.nfx",
        // `FUN_0026b2b0`.
        0x1d if legend => "xmtfblur.nfx",
        0x1d => "xmt0003.nfx",
        // `FUN_00269a58`, `FUN_002a3628`.
        0x21 | 0x57 => match from {
            0x1d | 0x2b => "xmt0004.nfx",
            0x1a => "xmtblur.nfx",
            _ => return None,
        },
        // `FUN_002cbe70`.
        0x27 => "xmt0005.nfx",
        // `FUN_0026e108`.
        0x29 if from == 0x0b => "xmtfblur.nfx",
        0x29 => "xmt0020.nfx",
        // `FUN_0026b3a0`.
        0x2b => "xmt0010.nfx",
        // `FUN_0029dfb0`.
        0x2c | 0x2d | 0x2f => match from {
            0x21 => "xmt0008.nfx",
            0x27 => "xmt0006.nfx",
            0x2b => "xmt0011.nfx",
            0x1d => "xmtfblur.nfx",
            _ => return None,
        },
        // `FUN_00281940`.
        0x35 | 0x62 if from == 0x06 => "xmt0007.nfx",
        _ => "xmtfblur.nfx",
    })
}

/// The movie a back change into `to` plays: `xmtblur.nfx` unless the
/// screen's slot `+0x174` refuses it for the screen before (the base
/// `FUN_002d8f60` allows it; Main's `FUN_0029f0d8`, Play It's
/// `FUN_002bfbb8`, Inside's `FUN_0028a7a8`, Bobbito's `FUN_00289860`,
/// Difficulty's `FUN_00285020`).
pub fn back_movie(to: u16, from: u16) -> Option<&'static str> {
    let none = match to {
        0x03 => matches!(from, 0x05 | 0x06 | 0x66),
        0x05 => from == 0x69,
        0x66 => matches!(from, 0x67 | 0x68),
        0x68 => from == 0x66,
        0x69 => from == 0x05,
        _ => false,
    };
    (!none).then_some("xmtblur.nfx")
}

/// The zoom onto the title's billboard (`FUN_002e1618`:
/// `movies/xmt0015.nfx`, `xmtjp15.nfx` in Japan; 31 frames, silent): the
/// last of the title's movies, and what a Start during its intro leaves.
pub const ZOOM: &str = "xmt0015.nfx";

/// The boot's movies: the first title's transition (`FUN_002ddb40` with the
/// front end's `+0x6c` clear, `0x2dfea8`; the French and Japanese consoles'
/// `xmtfr17`/`xmtfr16` and `xmtjp17`/`xmtjp16`, a PAL one's `xmtpl18`): the publisher
/// SPORTS BIG, the league's notice and its teams' logos, the intro and the
/// zoom. NuFX's logo is not among them (the credits play it).
pub const BOOT_REEL: [&str; 4] = ["xmt0017.nfx", "xmt0016.nfx", "xmt0018.nfx", ZOOM];

/// The title's movies once a title has stood (`0x2e0c1c`): the intro and
/// the zoom. The title's own slot (`FUN_002d41d0`) names nothing.
pub const TITLE_REEL: [&str; 2] = ["xmt0018.nfx", ZOOM];

/// The boot's movie the title's Start check waits for: its index 2, the
/// intro (`0x2de72c`: `FUN_0022e328` against 2 while `+0x6c` is clear).
const BOOT_INTRO: usize = 2;

/// From one movie's frames' end to the next one's first frame, ms. In the
/// project's PCSX2 runtime (2026-10-20) the boot's movie task opened each
/// of its movies 0.137, 0.127 and 0.083 s after the frames of the one
/// before, and each showed its first frame about 0.22 s after it opened
/// (the grabs: the publisher's frame 29 at 1.21 s, the intro's 295 at 10.03
/// s, the zoom's 8 at 0.50 s), the same lag as the first's; with the
/// title's settling this puts the boot's whole transition, 61.56 s, within
/// a few milliseconds.
pub const CHAIN_MS: f32 = 127.0;

/// From a Start the title's check takes to the zoom's first frame, ms: its
/// new movie widget opened the zoom 0.08 s after the press (boot 2), and
/// the zoom shows after the movies' usual hold.
pub const START_ZOOM_MS: f32 = 80.0 + 220.0;

/// From a Start on a DVD page to the change's end, ms (`FUN_002e14f8` gives
/// the transition a finished movie widget; measured 0.08 to 0.09 s on the
/// trailer, Bobbito's two, Play All and Take On The League).
pub const DVD_STOP_MS: f32 = 90.0;

/// Whether the screen of class `to` keeps the loop the screen before it
/// (reporting `from`) was playing: its slot `+0x15c` answers "sameloop"
/// (Main `FUN_0029f018` from Play It, My Team or Inside; Play It
/// `FUN_002bfb08` from Main or Difficulty; My Team `FUN_002a45f0` from
/// Main; Inside `FUN_0028a6f0` from Main, What's New or Bobbito; What's New
/// `FUN_0028b6a0` from Inside; Bobbito `FUN_002897b0` from Main or Inside;
/// Difficulty `FUN_00284f78` from Play It), and `FUN_002ddb40` leaves the
/// running loop alone; else it stops it and makes the screen's own anew as
/// the change ends (a DVD page after its own movie too: measured).
pub fn sameloop(to: u16, from: u16) -> bool {
    match to {
        0x03 => matches!(from, 0x05 | 0x06 | 0x66),
        0x05 => matches!(from, 0x03 | 0x69),
        0x06 => from == 0x03,
        0x66 => matches!(from, 0x03 | 0x67 | 0x68),
        0x67 => from == 0x66,
        0x68 => matches!(from, 0x03 | 0x66),
        0x69 => from == 0x05,
        _ => false,
    }
}

/// The DVD pages, whose transitions look for Start (`0x2de750`).
fn dvd_page(class: u16) -> bool {
    matches!(class, 0x66..=0x68)
}

/// The movie a change from the screen of class `from` to `to` plays. None
/// into the boot's screen (`FUN_002ddb40` returns at once for class 4).
pub fn movie(kind: Kind, from: u16, to: u16, legend: bool) -> Option<&'static str> {
    if reported(to) == 0x04 {
        return None;
    }
    match kind {
        Kind::Forward => forward_movie(to, reported(from), legend),
        Kind::Back => back_movie(to, reported(from)),
    }
}

/// The sound a transition's movie starts (`FUN_001a19b8` on its name): the
/// front end's event 2 with `k` (`FUN_001a1b88`, `FUN_0019e7c8`), and how
/// many front-end updates after the movie's start it fires (3 for `k` 0
/// and 1, at once for 2). By the name: a blur 0; `xmt00` then a digit
/// pair: 00, 07, 08, 11 and 20 give 2, 01 to 06, 09, 10, 12, 13, 14 and 19
/// give 1, the rest none.
pub fn whoosh(name: &str) -> Option<(i32, u32)> {
    let b = name.as_bytes();
    if !name.starts_with("xmt") || b.len() < 7 {
        return None;
    }
    let k = if b[3] == b'b' || b[4] == b'b' {
        0
    } else {
        match (b[5], b[6]) {
            (b'1', b'1') => 2,
            (b'1', b'0' | b'2' | b'3' | b'4' | b'9') => 1,
            (b'1', _) => return None,
            (b'2', b'0') => 2,
            (b'2', _) => return None,
            (_, b'0' | b'7' | b'8') => 2,
            _ => 1,
        }
    };
    Some((k, if k == 2 { 0 } else { 3 }))
}

/// Whether the old screen's keys play the transition sound 0 themselves
/// as they ask for a change with no movie (`FUN_001a1b88(.., 0)` beside
/// `FUN_002d7cb0`/`FUN_002d7d40`, the same update): Main's Play It, My
/// Team and Inside Vol. 2 (`FUN_0029e700`), Play It's Pick Up Game, league
/// Challenge and Be A Legend and its Triangle (`FUN_002bf428`),
/// Difficulty's Triangle (`FUN_00284848`), My Team's Triangle
/// (`FUN_002a3fe0`), Inside's What's New and Bobbito rows and its Triangle
/// (`FUN_00289f98`), What's New's and Bobbito's Triangle (`FUN_0028aee0`,
/// `FUN_00289100`). Select's Yes calls it on Inside alone
/// (`crate::exit_ask::whooshes`).
pub fn hand_whoosh(from: u16, to: u16, kind: Kind) -> bool {
    match kind {
        Kind::Forward => matches!(
            (from, to),
            (0x03, 0x05 | 0x06 | 0x66) | (0x05, 0x69) | (0x66, 0x67 | 0x68)
        ),
        Kind::Back => matches!(
            (from, to),
            (0x05 | 0x06 | 0x66, 0x03) | (0x69, 0x05) | (0x67 | 0x68, 0x66)
        ),
    }
}

/// What a widget does in a transition record.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Effect {
    /// Its alpha from 0 to 255 over the record (flag 0x10).
    FadeIn,
    /// From 255 to 0 (flag 0x20).
    FadeOut,
    /// From off the left edge to its place at the record's speed (flag 4,
    /// side 2).
    SlideIn,
    /// From its place off the left edge (flag 8, side 2).
    SlideOut,
}

/// A widget's transition record (`FUN_002d9060`'s table from `0x3fdf58`,
/// read from the runtime's RAM; `FUN_0024a9c0` starts it, `FUN_00249cb0`
/// runs it, `FUN_00249b88` ends it): from `start` to `end` ms of the
/// screen's state, `speed` pixels a millisecond for a slide.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Record {
    pub start: f32,
    pub end: f32,
    pub effect: Effect,
    pub speed: f32,
}

/// `0x3fdf58`: arriving widgets fade in over 166.7 ms.
pub const FADE_IN: Record = Record {
    start: 0.0,
    end: 166.666_67,
    effect: Effect::FadeIn,
    speed: 0.0,
};
/// `0x3fdf78`: leaving widgets fade out over 166.7 ms.
pub const FADE_OUT: Record = Record {
    start: 0.0,
    end: 166.666_67,
    effect: Effect::FadeOut,
    speed: 0.0,
};
/// `0x3fdf98`: a leaving title slides off to the left at 2.4 px/ms.
pub const SLIDE_OUT: Record = Record {
    start: 0.0,
    end: 333.333_34,
    effect: Effect::SlideOut,
    speed: 2.4,
};
/// `0x3fe0f8`: an arriving title slides in from the left.
pub const SLIDE_IN: Record = Record {
    start: 0.0,
    end: 266.666_66,
    effect: Effect::SlideIn,
    speed: 2.4,
};

/// A screen's records for one state: its title widget's and its other
/// widgets' (the elements its build lists; the backdrop, the poster in it
/// and the city loop over it are not widgets of the screen and stay).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Records {
    pub title: Record,
    pub rest: Record,
}

/// The screens over the main backdrop with the city loop, whose builds
/// give the title (element 19, 20, 23 or 0) its own records and every
/// other listed widget the fades: Play It `FUN_002beef8` (tables
/// `0x48f1a0`, `0x48f1e8`), My Team `FUN_002a3c60`, Difficulty
/// `FUN_00284280`, Inside `FUN_00289a10`, What's New `FUN_0028a970`,
/// Bobbito `FUN_00288be0`.
fn main_backdrop(class: u16) -> bool {
    matches!(class, 0x05 | 0x06 | 0x66..=0x69)
}

/// The records a screen's widgets play arriving (state 0).
pub fn arriving(class: u16) -> Option<Records> {
    match class {
        c if main_backdrop(c) => Some(Records {
            title: SLIDE_IN,
            rest: FADE_IN,
        }),
        // `FUN_002d83a8(screen, 0, 0x3fdf58)`, every child: Main
        // (`FUN_0029e230`), Choose Court (`0x267f10`), Choose Players and
        // the League Challenge's (`FUN_00269c08`), the Choose Team family
        // (`0x26f568`, `0x270108`, `0x270f50`, `0x271730`), Update Team
        // (`0x2d65d4`).
        0x03 | 0x19 | 0x1b | 0x1c | 0x1d | 0x2b | 0x21 | 0x4d | 0x60 => Some(Records {
            title: FADE_IN,
            rest: FADE_IN,
        }),
        _ => None,
    }
}

/// The records a screen's widgets play leaving (state 1 or 2). Main has a
/// table for state 1 only (`0x4894b0`, element 27 the title).
pub fn leaving(class: u16, kind: Kind) -> Option<Records> {
    let slides = main_backdrop(class) || (class == 0x03 && kind == Kind::Forward);
    slides.then_some(Records {
        title: SLIDE_OUT,
        rest: FADE_OUT,
    })
}

/// A screen's deadline for state 0 in ms (`FUN_002d8398(t, screen, 0)`):
/// its widgets arrive until then, and the queued presses reach it after.
pub fn arriving_ms(class: u16) -> f32 {
    match class {
        // `DAT_003fdf50`.
        0x03 => 166.666_67,
        // `DAT_003fdf48`.
        c if main_backdrop(c) => 266.666_66,
        // 167.0 (`lui 0x4327`): Choose Players and its base (`0x269b68`,
        // `0x26b018`), Choose Court (`0x267c5c`), the Choose Team family's
        // base `FUN_0026e1a8` (`0x26e28c`), the brackets (`0x2cbf70`,
        // `0x2852bc`, `0x265fe0`), Update Team (`0x2d61f8`), Choose Region
        // (its own, inside `FUN_0026e1a8`'s range).
        0x19 | 0x1b | 0x1c | 0x4d | 0x1d | 0x2b | 0x21 | 0x29 | 0x58 | 0x59 | 0x5e | 0x60 => 167.0,
        _ => 0.0,
    }
}

/// A screen's deadline for leaving in ms (`FUN_002d8398(t, screen, 1 | 2)`,
/// `DAT_003fdf4c`; Main sets state 1's only).
pub fn leaving_ms(class: u16, kind: Kind) -> f32 {
    if main_backdrop(class) || (class == 0x03 && kind == Kind::Forward) {
        266.666_66
    } else {
        0.0
    }
}

/// From the front end's request to the movie's first frame, ms (measured:
/// 210 to 250 over Options to Main, User Setup to Choose Team, Choose
/// Players to Choose Court, High Scores to Main; two updates when there is
/// no movie, Main to Play It), and the new screen's own build on top where
/// it is slow (Options 90, the loading screen's picture 380).
pub fn holding_ms(to: u16, movie: bool) -> f32 {
    (if movie { 220.0 } else { 33.0 })
        + match to {
            0x28 => 90.0,
            0x2c | 0x2d | 0x2f => 380.0,
            _ => 0.0,
        }
}

/// From the movie's end to the new screen's first update, ms (measured:
/// the time its arrival takes, from the transition's end to state 4 less
/// the deadline): the city loop's load on Main from a screen without it
/// (`FUN_002ddb40` makes `xlp0005.nfx` there; `sameloop` keeps it),
/// the Choose Team family's logos, User Setup's columns; two updates
/// elsewhere.
pub fn settling_ms(to: u16, from: u16) -> f32 {
    match to {
        // The title's billboard (`FUN_00235a18` on the front end's picture
        // widget, waited for once the movies are done): the zoom's frames'
        // end to the title's state 4 0.13 s (0.10 to 0.17 over the boots'
        // zooms and Starts).
        0x63 => 130.0,
        // A DVD page after its own movie: the city loop made anew (the page
        // asks for it afresh from itself) and the page's own build; from
        // the end to state 4 0.53 to 0.58 s (five movies).
        c if dvd_page(c) && c == from => 300.0,
        // Main makes its city loop anew from any screen that does not keep
        // it (Difficulty and the DVD pages share its backdrop but not its
        // loop: measured 0.11 s from them, Select's Yes).
        0x03 if !sameloop(0x03, reported(from)) => 140.0,
        0x19 | 0x1b | 0x1c | 0x4d => 130.0,
        0x1d | 0x2b | 0x21 => 120.0,
        0x0c..=0x13 => 170.0,
        _ => 30.0,
    }
}

/// The movies' rate (`XMT*`: 29.97 frames a second).
pub const MOVIE_RATE: f64 = 30_000.0 / 1001.0;

/// Poses a screen's pieces `t` ms into a state with `records`: its title's
/// pieces slide, the other widgets' fade, the fixed ones stay.
pub fn pose(pieces: &mut [Piece], records: &Records, t: f32) {
    // The title's right edge: a slide moves it off until that edge is 6
    // pixels past the left (`FUN_0024a9c0` side 2: the target is the
    // widget's width and 1 short of 0, then 5 more).
    let right = pieces
        .iter()
        .filter(|p| p.role == Role::Title)
        .map(|p| p.rect.max.x)
        .fold(f32::NEG_INFINITY, f32::max);
    let travel = right + 6.0;
    for p in pieces.iter_mut() {
        let record = match p.role {
            Role::Title => &records.title,
            Role::Widget => &records.rest,
            Role::Fixed => continue,
        };
        match record.effect {
            Effect::FadeIn | Effect::FadeOut => {
                let a = fade(record, t);
                p.colour = p.colour.with_alpha(p.colour.alpha() * a);
            }
            Effect::SlideIn | Effect::SlideOut => {
                let dx = slide(record, t, travel);
                p.rect.min.x += dx;
                p.rect.max.x += dx;
            }
        }
    }
}

/// A fade's alpha `t` ms in (`FUN_00249cb0`: (t - start) / (end - start) of
/// 255 in, the rest out; held at the end, and before the start the
/// state's first update has set it).
fn fade(r: &Record, t: f32) -> f32 {
    let f = ((t - r.start) / (r.end - r.start)).clamp(0.0, 1.0);
    match r.effect {
        Effect::FadeIn => f,
        _ => 1.0 - f,
    }
}

/// A slide's offset `t` ms in: the widget moves `speed` px/ms from its
/// start until it passes its target or the record ends, then stands at the
/// target.
fn slide(r: &Record, t: f32, travel: f32) -> f32 {
    let moved = if t >= r.end {
        travel
    } else {
        (r.speed * (t - r.start).max(0.0)).min(travel)
    };
    match r.effect {
        Effect::SlideOut => -moved,
        _ => moved - travel,
    }
}

/// A change of screen: the classes either side (the factory's), its kind,
/// whether the front end is in Be A Legend, and the game state that
/// follows its movie (into a game's load, or back to the menus after
/// one).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Change {
    pub from: u16,
    pub to: u16,
    pub kind: Kind,
    pub legend: bool,
    pub screen: Option<Screen>,
    /// A DVD page's movies, played as the page goes forward into itself:
    /// its keys set the name (`DAT_003fdf44`; "xmtall" for What's New's
    /// Play All) and the page built anew names it (`FUN_002ddb40`, the
    /// four for "xmtall").
    pub dvd: Option<&'static [&'static str]>,
    /// Ms the front end takes before the movie on top of the new screen's
    /// hold ([`holding_ms`]): what the old screen's leaving costs where it
    /// was measured (Select's Yes, [`crate::exit_ask::hold_ms`]).
    pub hold_ms: f32,
    /// The old screen's keys play the transition sound themselves as they
    /// ask for the change ([`hand_whoosh`]).
    pub whoosh: bool,
}

impl Change {
    /// A change between two screens, playing their slots' movie.
    pub fn new(from: u16, to: u16, kind: Kind, legend: bool, screen: Option<Screen>) -> Self {
        Self {
            from,
            to,
            kind,
            legend,
            screen,
            dvd: None,
            hold_ms: 0.0,
            whoosh: false,
        }
    }

    pub fn movie(&self) -> Option<&'static str> {
        movie(self.kind, self.from, self.to, self.legend)
    }

    /// Into the title, forward: its own movies ([`BOOT_REEL`],
    /// [`TITLE_REEL`]).
    fn titles(&self) -> bool {
        self.kind == Kind::Forward && self.to == 0x63 && self.dvd.is_none()
    }
}

/// The movies a change plays, one after another (the movie task's queue).
pub fn reel(change: &Change, titled: bool) -> Vec<&'static str> {
    if let Some(files) = change.dvd {
        return files.to_vec();
    }
    if change.titles() {
        return if titled {
            TITLE_REEL.to_vec()
        } else {
            BOOT_REEL.to_vec()
        };
    }
    let mut names: Vec<&'static str> = change.movie().into_iter().collect();
    // `FUN_002ddb40` queues the blur after the slot's movie for a screen
    // reporting class 7 (the credits).
    if change.kind == Kind::Forward && reported(change.to) == 0x07 {
        names.push("xmtfblur.nfx");
    }
    names
}

/// Whether a movie's soundtrack plays at the music's level: all but the publisher
/// SPORTS BIG's and NuFX's logos, which `FUN_0022d958` leaves at the movie
/// player's full level (no `FUN_00346f68` for them; the rest take the
/// music's volume, `FUN_001a1720(.., 2)`, the path manager's).
pub fn at_music_level(name: &str) -> bool {
    !matches!(name, "xmt0017.nfx" | "xmtjp17.nfx" | "xnufx.nfx")
}

/// A transition movie's soundtrack playing.
#[derive(Component)]
pub struct TransitionSound;

/// A press waiting for the new screen.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Press {
    Key(KeyCode),
    Pad(Entity, GamepadButton),
}

/// At most this many presses wait.
const QUEUE: usize = 8;

/// What a Start does during a change's movies (`FUN_002e14f8`).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Skip {
    /// The title's: the movie on show ends and the zoom plays alone.
    Zoom,
    /// A DVD page's: the change ends.
    End,
}

/// A change's movies: the names in order, the one on show (`index`, the
/// task's `+0x199`) and the next, opened ahead.
struct Reel {
    names: Vec<&'static str>,
    index: usize,
    clip: Option<movie::TransitionClip>,
    next: Option<movie::TransitionClip>,
    /// When the clip at `index` is due to show its first frame.
    due: f64,
    /// When the clip on show ended (its last frame's time over).
    ended: Option<f64>,
    /// The boot's movies (the front end's `+0x6c` clear when it began).
    boot: bool,
}

impl Reel {
    fn name(&self) -> Option<&'static str> {
        self.names.get(self.index).copied()
    }
}

#[derive(Default)]
enum Phase {
    #[default]
    Idle,
    Leaving {
        since: f64,
        change: Change,
    },
    Holding {
        change: Change,
        reel: Reel,
    },
    Playing {
        reel: Reel,
        overlay: Entity,
        change: Change,
    },
    Arriving {
        since: f64,
        change: Change,
    },
}

/// The transition under way, and the screen's last pieces it holds.
#[derive(Resource, Default)]
pub struct Transition {
    phase: Phase,
    /// The page's own pieces as last drawn (not the card's or the Help
    /// overlay's): the leaving screen's picture.
    shown: Vec<Piece>,
    queue: Vec<Press>,
    injected: Vec<Press>,
    /// The movie's sound, due at this time.
    sound: Option<(f64, i32)>,
    /// The overlay to remove once the state the movie led to has drawn.
    lift: Option<Entity>,
    /// A Start the change's check took, for the next update.
    skip: Option<Skip>,
    /// A title has stood (the front end's `+0x6c`, set as the first
    /// title's transition ends, `0x2de07c`): later titles play
    /// [`TITLE_REEL`].
    pub titled: bool,
    /// The title's movies still take a Start (the front end's `+0x70`: set
    /// as a later title's movies start, `0x2e10c4`, cleared by the Start it
    /// takes, `0x2de7ac`).
    armed: bool,
    /// Seconds of real time with no transition under way: scripted key
    /// presses and screenshots count it, so a script reaches the same
    /// pages whatever the transitions take.
    pub clock: f64,
    /// The screen class the menus showed last update.
    pub(crate) last: Option<(u16, usize)>,
    /// When each transition of the run began (real seconds), for a
    /// screenshot timed from one (`--at-transition`).
    pub starts: Vec<f64>,
}

impl Transition {
    /// Nothing under way: the screen takes presses.
    pub fn idle(&self) -> bool {
        matches!(self.phase, Phase::Idle)
    }

    /// The screen on show stands: no transition, or the new screen
    /// arriving (its slot `+0x16c` runs as the movie ends,
    /// `FUN_002dcff8`/`FUN_002dd238`).
    pub fn standing(&self) -> bool {
        matches!(self.phase, Phase::Idle | Phase::Arriving { .. })
    }

    /// The front end's song waits: the title's movies until the zoom opens
    /// (`FUN_0022d958` on `xmt0015`: `FUN_0019fd60(.., -5, 0)` and
    /// `FUN_0019ff60`; the boot's song started there, and the title's turn
    /// stops it, `FUN_0019fd60(.., -5, 0)`), and a DVD page's movies from
    /// the press (the page's keys stop it) to the change's end.
    pub fn holds_song(&self) -> bool {
        match &self.phase {
            Phase::Leaving { change, .. } => change.titles() || change.dvd.is_some(),
            Phase::Holding { change, reel } | Phase::Playing { change, reel, .. } => {
                change.dvd.is_some() || (change.titles() && reel.name() != Some(ZOOM))
            }
            _ => false,
        }
    }

    /// The pages draw: not while the old screen's picture holds.
    pub fn draws(&self) -> bool {
        !matches!(self.phase, Phase::Leaving { .. } | Phase::Holding { .. })
    }

    /// The city loop runs: not while the frame holds or a movie covers it.
    pub fn loop_runs(&self) -> bool {
        !matches!(self.phase, Phase::Holding { .. } | Phase::Playing { .. })
    }

    /// A list screen's lit row shows: not while the screen arrives (Main,
    /// Play It and the rest over the main backdrop light theirs in state
    /// 4).
    pub fn rows_lit(&self) -> bool {
        match &self.phase {
            Phase::Playing { change, .. } | Phase::Arriving { change, .. } => {
                !(change.to == 0x03 || main_backdrop(change.to))
            }
            _ => true,
        }
    }

    /// Keeps the page's pieces as drawn this update.
    pub fn keep(&mut self, pieces: &[Piece]) {
        self.shown.clear();
        self.shown.extend_from_slice(pieces);
    }

    /// Starts `change` at `now`.
    pub fn begin(&mut self, change: Change, now: f64) {
        self.queue.clear();
        self.skip = None;
        if change.whoosh {
            self.sound = Some((now, 0));
        }
        self.starts.push(now);
        self.armed = change.titles() && self.titled;
        self.phase = Phase::Leaving { since: now, change };
    }

    /// What a Start does now: the change's check runs while its movies
    /// play (from its first update to their end; `0x2de71c`).
    fn start_check(&self) -> Option<Skip> {
        let (change, reel) = match &self.phase {
            Phase::Holding { change, reel } | Phase::Playing { change, reel, .. } => (change, reel),
            _ => return None,
        };
        if change.titles() {
            let takes = if reel.boot {
                reel.index == BOOT_INTRO
            } else {
                self.armed
            };
            takes.then_some(Skip::Zoom)
        } else if dvd_page(change.to) {
            Some(Skip::End)
        } else {
            None
        }
    }

    /// Poses the arriving screen's pieces (hidden under its movie, then its
    /// records from its first update).
    pub fn arrive(&self, pieces: &mut [Piece], now: f64) {
        let (change, t) = match &self.phase {
            Phase::Playing { change, .. } => (change, f32::NEG_INFINITY),
            Phase::Arriving { since, change } => (
                change,
                ((now - since) * 1000.0) as f32 - settling_ms(change.to, change.from),
            ),
            _ => return,
        };
        if let Some(records) = arriving(change.to) {
            pose(pieces, &records, t.max(-1.0));
        }
    }
}

/// The run condition: nothing under way.
pub fn idle(t: Res<Transition>) -> bool {
    t.idle()
}

/// The run condition: the pages draw.
pub fn draws(t: Res<Transition>) -> bool {
    t.draws()
}

/// The run condition: the city loop runs.
pub fn loop_runs(t: Res<Transition>) -> bool {
    t.loop_runs()
}

/// Opens `/MOVIES/<name>` for a change's reel.
fn open(
    name: &str,
    disc: &mut Option<ResMut<Disc>>,
    images: &mut Assets<Image>,
) -> Option<movie::TransitionClip> {
    let disc = disc.as_deref_mut()?;
    movie::TransitionClip::open(name, disc, images)
}

/// A Start the check took (`FUN_002e14f8`): on the title a movie widget
/// with the zoom alone takes the one playing (`FUN_002e1618`,
/// `FUN_0022e1b0`), and the title's later movies take no other Start (its
/// `+0x70` cleared); on a DVD page a finished one, so the change ends.
fn take_skip(
    skip: Skip,
    reel: &mut Reel,
    now: f64,
    disc: &mut Option<ResMut<Disc>>,
    images: &mut Assets<Image>,
) {
    match skip {
        Skip::Zoom => {
            reel.names = vec![ZOOM];
            reel.index = 0;
            reel.clip = open(ZOOM, disc, images);
            reel.next = None;
            reel.due = now + f64::from(START_ZOOM_MS) / 1000.0;
            reel.ended = None;
        }
        Skip::End => {
            reel.names.truncate(reel.index + 1);
            reel.next = None;
            // Over once the stop's time has passed (the end waits a frame's
            // time, below).
            reel.ended = Some(now + f64::from(DVD_STOP_MS) / 1000.0 - 1.0 / MOVIE_RATE);
        }
    }
}

/// Starts a clip's soundtrack with its first frame: at the music's level,
/// the logos at their own ([`at_music_level`]).
fn sound_of(
    clip: &mut movie::TransitionClip,
    name: &str,
    commands: &mut Commands,
    pcm: Option<&mut Assets<crate::audio::Pcm>>,
    settings: Option<&crate::audio::AudioSettings>,
) {
    if let (Some(sound), Some(pcm), Some(settings)) = (clip.take_sound(), pcm, settings) {
        info!("audio: {name}'s soundtrack");
        crate::audio::play_movie(
            commands,
            pcm,
            settings,
            sound,
            at_music_level(name),
            TransitionSound,
        );
    }
}

/// Runs the transition: the leaving screen's records over its held
/// picture, the hold, the movies over the whole frame one after another,
/// the state they lead to, the arriving screen's records.
#[allow(clippy::too_many_arguments)]
pub fn advance(
    mut t: ResMut<Transition>,
    time: Res<Time<Real>>,
    mut commands: Commands,
    (mut disc, mut images): (Option<ResMut<Disc>>, ResMut<Assets<Image>>),
    mut slots: Query<(&look::Slot, &mut ImageNode, &mut Node, &mut Visibility)>,
    state: Res<State<Screen>>,
    mut state_next: ResMut<NextState<Screen>>,
    (sfx, mut film): (
        Option<ResMut<crate::sfx::MenuSfx>>,
        Option<ResMut<movie::FilmRoom>>,
    ),
    (mut pcm, settings, sounds): (
        Option<ResMut<Assets<crate::audio::Pcm>>>,
        Option<Res<crate::audio::AudioSettings>>,
        Query<Entity, With<TransitionSound>>,
    ),
) {
    let now = time.elapsed_secs_f64();
    let t = &mut *t;
    let film = film.as_deref_mut();
    if t.idle() {
        t.clock += f64::from(time.delta_secs());
    }
    if let Some(overlay) = t.lift.take() {
        commands.entity(overlay).despawn();
    }
    if let (Some((at, k)), Some(mut sfx)) = (t.sound, sfx)
        && now >= at
    {
        sfx.transition(k);
        t.sound = None;
    }
    let hush = |commands: &mut Commands| {
        for sound in &sounds {
            commands.entity(sound).despawn();
        }
    };
    let last = 1.0 / MOVIE_RATE;
    match std::mem::take(&mut t.phase) {
        Phase::Idle => {}
        Phase::Leaving { since, change } => {
            let ms = ((now - since) * 1000.0) as f32;
            let from = change.from;
            if let Some(records) = leaving(from, change.kind) {
                let mut pieces = t.shown.clone();
                pose(&mut pieces, &records, ms);
                look::show(&mut commands, &mut slots, &pieces, *state.get());
            }
            // The deadline passed at an update; the front end takes the
            // request at its next.
            if ms > leaving_ms(from, change.kind) {
                let names = reel(&change, t.titled);
                let clip = names
                    .first()
                    .and_then(|name| open(name, &mut disc, &mut images));
                let next = names
                    .get(1)
                    .and_then(|name| open(name, &mut disc, &mut images));
                // The sound counts front-end updates from the movie's start.
                t.sound = names
                    .first()
                    .and_then(|name| whoosh(name))
                    .map(|(k, updates)| (now + f64::from(updates + 1) / 60.0, k));
                let hold = holding_ms(change.to, !names.is_empty()) + change.hold_ms;
                t.phase = Phase::Holding {
                    reel: Reel {
                        boot: change.titles() && !t.titled,
                        names,
                        index: 0,
                        clip,
                        next,
                        due: now + f64::from(hold) / 1000.0,
                        ended: None,
                    },
                    change,
                };
            } else {
                t.phase = Phase::Leaving { since, change };
            }
        }
        Phase::Holding { change, mut reel } => {
            if let Some(skip) = t.skip.take() {
                if skip == Skip::End {
                    reel.names.clear();
                    reel.due = now + f64::from(DVD_STOP_MS) / 1000.0;
                } else {
                    take_skip(skip, &mut reel, now, &mut disc, &mut images);
                }
            }
            if now < reel.due {
                t.phase = Phase::Holding { change, reel };
            } else if let (Some(name), Some(clip)) = (reel.name(), reel.clip.as_mut()) {
                // Its clock starts with its first frame on show.
                if !clip.shown() {
                    clip.start(now);
                }
                clip.step(now, &mut images);
                let image = clip.shown().then(|| clip.image());
                if image.is_some() {
                    sound_of(
                        clip,
                        name,
                        &mut commands,
                        pcm.as_deref_mut(),
                        settings.as_deref(),
                    );
                }
                match image {
                    Some(image) => {
                        let overlay = commands
                            .spawn((
                                ImageNode::new(image).with_mode(NodeImageMode::Stretch),
                                Node {
                                    position_type: PositionType::Absolute,
                                    left: Val::Percent(0.0),
                                    top: Val::Percent(0.0),
                                    width: Val::Percent(100.0),
                                    height: Val::Percent(100.0),
                                    ..default()
                                },
                                GlobalZIndex(i32::MAX - 16),
                            ))
                            .id();
                        t.phase = Phase::Playing {
                            reel,
                            overlay,
                            change,
                        };
                    }
                    None => t.phase = Phase::Holding { change, reel },
                }
            } else {
                // No movie, or one the disc could not give: the change goes
                // on without it.
                arrive(t, change, now, None, &mut state_next, film);
            }
        }
        Phase::Playing {
            mut reel,
            overlay,
            change,
        } => {
            if let Some(skip) = t.skip.take() {
                take_skip(skip, &mut reel, now, &mut disc, &mut images);
                hush(&mut commands);
            }
            if now >= reel.due {
                let name = reel.name().unwrap_or_default();
                match reel.clip.as_mut() {
                    Some(clip) => {
                        let was = clip.shown();
                        if !was {
                            clip.start(now);
                        }
                        clip.step(now, &mut images);
                        if clip.shown() && !was {
                            // The next movie in the picture, its sound with
                            // it.
                            commands.entity(overlay).insert(
                                ImageNode::new(clip.image()).with_mode(NodeImageMode::Stretch),
                            );
                            sound_of(
                                clip,
                                name,
                                &mut commands,
                                pcm.as_deref_mut(),
                                settings.as_deref(),
                            );
                        }
                        if reel.ended.is_none() && clip.ended() {
                            reel.ended = Some(now);
                        }
                    }
                    None => {
                        reel.ended.get_or_insert(now - last);
                    }
                }
            }
            match reel.ended {
                // The last frame stands its own time; the next movie opens
                // then and shows after the chain's gap.
                Some(at) if now - at >= last => {
                    hush(&mut commands);
                    if reel.index + 1 < reel.names.len() {
                        reel.index += 1;
                        let name = reel.names[reel.index];
                        reel.clip = reel
                            .next
                            .take()
                            .or_else(|| open(name, &mut disc, &mut images));
                        reel.next = reel
                            .names
                            .get(reel.index + 1)
                            .and_then(|name| open(name, &mut disc, &mut images));
                        reel.due = now + f64::from(CHAIN_MS) / 1000.0;
                        reel.ended = None;
                        t.phase = Phase::Playing {
                            reel,
                            overlay,
                            change,
                        };
                    } else {
                        arrive(t, change, now, Some(overlay), &mut state_next, film);
                    }
                }
                _ => {
                    t.phase = Phase::Playing {
                        reel,
                        overlay,
                        change,
                    }
                }
            }
        }
        Phase::Arriving { since, change } => {
            let ms = ((now - since) * 1000.0) as f32;
            if ms >= settling_ms(change.to, change.from) + arriving_ms(change.to) {
                t.phase = Phase::Idle;
            } else {
                t.phase = Phase::Arriving { since, change };
            }
        }
    }
}

/// The movies over: the state they lead to, and the new screen's arrival.
/// The overlay stays until that state has drawn an update. A title's
/// arrival marks that one has stood (the front end's `+0x6c`); the new
/// screen's loop starts afresh unless it keeps the one before
/// ([`sameloop`]).
fn arrive(
    t: &mut Transition,
    change: Change,
    now: f64,
    overlay: Option<Entity>,
    next: &mut NextState<Screen>,
    film: Option<&mut movie::FilmRoom>,
) {
    if let Some(screen) = change.screen {
        next.set(screen);
    }
    if let Some(film) = film
        && !sameloop(change.to, reported(change.from))
    {
        film.restart();
    }
    if change.titles() {
        t.titled = true;
    }
    t.armed = false;
    t.lift = overlay;
    t.phase = Phase::Arriving { since: now, change };
}

/// Presses during a transition. While the old screen leaves they reach it
/// (it drops them). While the movies play the front end empties its pad
/// events every update (`0x2de7b8`): none reaches the new screen, but a
/// Start the change's check takes ([`Transition::start_check`]). While the
/// new screen arrives they wait for it and reach it one an update once it
/// stands. Runs after the input's systems and before the pads' menu
/// directions.
pub fn queue_presses(
    mut t: ResMut<Transition>,
    mut keys: ResMut<ButtonInput<KeyCode>>,
    mut pads: Query<(Entity, &mut Gamepad)>,
) {
    let t = &mut *t;
    for press in t.injected.drain(..) {
        match press {
            Press::Key(k) => keys.release(k),
            Press::Pad(e, b) => {
                if let Ok((_, mut pad)) = pads.get_mut(e) {
                    pad.digital_mut().release(b);
                }
            }
        }
    }
    let check = t.start_check();
    match t.phase {
        Phase::Holding { .. } | Phase::Playing { .. } | Phase::Arriving { .. } => {
            let arriving = matches!(t.phase, Phase::Arriving { .. });
            let mut pressed: Vec<Press> = keys.get_just_pressed().map(|&k| Press::Key(k)).collect();
            for (e, pad) in &pads {
                pressed.extend(pad.get_just_pressed().map(|&b| Press::Pad(e, b)));
            }
            for p in pressed {
                // Start (Enter, a pad's Start) for the check.
                let start = matches!(
                    p,
                    Press::Key(KeyCode::Enter) | Press::Pad(_, GamepadButton::Start)
                );
                if arriving {
                    if t.queue.len() < QUEUE {
                        t.queue.push(p);
                    }
                } else if start && t.skip.is_none() && check.is_some() {
                    t.skip = check;
                    if check == Some(Skip::Zoom) {
                        t.armed = false;
                    }
                }
            }
            // Nothing reaches the screens meanwhile.
            keys.clear();
            for (_, mut pad) in &mut pads {
                let buttons: Vec<GamepadButton> = pad.get_just_pressed().copied().collect();
                for b in buttons {
                    pad.digital_mut().clear_just_pressed(b);
                }
            }
        }
        Phase::Leaving { .. } => {}
        Phase::Idle => {
            if t.queue.is_empty() {
                return;
            }
            let press = t.queue.remove(0);
            match press {
                Press::Key(k) => {
                    keys.release(k);
                    keys.press(k);
                }
                Press::Pad(e, b) => {
                    if let Ok((_, mut pad)) = pads.get_mut(e) {
                        pad.digital_mut().release(b);
                        pad.digital_mut().press(b);
                    }
                }
            }
            t.injected.push(press);
        }
    }
}

pub struct TransitionPlugin;

impl Plugin for TransitionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Transition>()
            .add_systems(
                PreUpdate,
                queue_presses
                    .after(bevy::input::InputSystems)
                    .before(crate::pads::NavSystems),
            )
            .add_systems(Update, advance.in_set(TransitionSystems));
    }
}

/// [`advance`]: after the screens' own updates in the frame.
#[derive(SystemSet, Clone, Debug, PartialEq, Eq, Hash)]
pub struct TransitionSystems;

#[cfg(test)]
mod tests {
    use super::*;

    /// The movies each pair plays, as the slots give them and as the
    /// runtime played them (2026-10-20): Title (the attract, 99) to Main
    /// xmt0000, Main to Options and to High Scores xmtfblur, Options and
    /// High Scores back to Main xmtblur, Main to Play It and back nothing,
    /// Play It to Difficulty nothing, Difficulty to User Setup xmt0001,
    /// User Setup to Choose Team xmt0002, Choose Team to Choose Players
    /// xmt0003, Choose Players to Choose Court xmt0004, Choose Court to the
    /// game's load xmt0008.
    #[test]
    fn the_pairs_play_the_movies_the_runtime_played() {
        use Kind::{Back, Forward};
        let cases = [
            (Forward, 0x63, 0x03, Some("xmt0000.nfx")),
            (Forward, 0x03, 0x28, Some("xmtfblur.nfx")),
            (Back, 0x28, 0x03, Some("xmtblur.nfx")),
            (Forward, 0x03, 0x0a, Some("xmtfblur.nfx")),
            (Back, 0x0a, 0x03, Some("xmtblur.nfx")),
            (Forward, 0x03, 0x05, None),
            (Back, 0x05, 0x03, None),
            (Forward, 0x05, 0x69, None),
            (Back, 0x69, 0x05, None),
            (Forward, 0x69, 0x0d, Some("xmt0001.nfx")),
            (Forward, 0x0d, 0x1b, Some("xmt0002.nfx")),
            (Forward, 0x1b, 0x1d, Some("xmt0003.nfx")),
            (Forward, 0x1d, 0x21, Some("xmt0004.nfx")),
            (Forward, 0x21, 0x2c, Some("xmt0008.nfx")),
        ];
        for (kind, from, to, want) in cases {
            assert_eq!(movie(kind, from, to, false), want, "{from:#x} to {to:#x}");
        }
        // Be A Legend's Choose Players, and the League Challenge's.
        assert_eq!(movie(Kind::Forward, 0x31, 0x1d, true), Some("xmtfblur.nfx"));
        assert_eq!(movie(Kind::Forward, 0x29, 0x2b, false), Some("xmt0010.nfx"));
        assert_eq!(movie(Kind::Forward, 0x2b, 0x2c, false), Some("xmt0011.nfx"));
        // Region from User Setup (reported 0xb) blurs; from elsewhere 0020.
        assert_eq!(
            movie(Kind::Forward, 0x0e, 0x29, false),
            Some("xmtfblur.nfx")
        );
        assert_eq!(movie(Kind::Forward, 0x19, 0x29, false), Some("xmt0020.nfx"));
        // My Team's crew, Create Team elsewhere; the join-a-team's way home.
        assert_eq!(movie(Kind::Forward, 0x06, 0x35, false), Some("xmt0007.nfx"));
        assert_eq!(movie(Kind::Forward, 0x36, 0x62, true), Some("xmtfblur.nfx"));
        assert_eq!(movie(Kind::Forward, 0x4d, 0x03, false), Some("xmt0013.nfx"));
        // The DVD pages among themselves, and into the boot screen.
        assert_eq!(movie(Kind::Back, 0x67, 0x66, false), None);
        assert_eq!(movie(Kind::Back, 0x67, 0x03, false), Some("xmtblur.nfx"));
        assert_eq!(movie(Kind::Forward, 0x63, 0x04, false), None);
    }

    /// The model's time for a reel: the hold before its first movie, each
    /// movie's frames, the chain's gap before each next one.
    fn reel_ms(hold: f32, frames: &[f64]) -> f32 {
        let clips: f32 = frames
            .iter()
            .map(|&n| (n / MOVIE_RATE * 1000.0) as f32)
            .sum();
        hold + clips + CHAIN_MS * (frames.len() - 1) as f32
    }

    /// The boot's movies as the first title's transition (three boots of
    /// the runtime, 2026-10-20): the publisher (75 frames), the league's
    /// notice (399), the intro (1,318) and the zoom (31), no NuFX; from the
    /// title's class showing (count 1,913) to its state 4 (8,069; the third
    /// boot 1,912 to 8,069) 61.56 s, the model within 20 ms; the intro's
    /// first frame 16.26 s in and the zoom's 60.38 s (from the opens and
    /// the grabbed frames), the model within 50 ms.
    #[test]
    fn the_boot_plays_its_movies_as_the_first_titles_transition() {
        let change = Change::new(0x04, 0x63, Kind::Forward, false, None);
        assert_eq!(
            reel(&change, false),
            vec!["xmt0017.nfx", "xmt0016.nfx", "xmt0018.nfx", "xmt0015.nfx"]
        );
        assert!(!reel(&change, false).contains(&"xnufx.nfx"));
        let hold = holding_ms(0x63, true);
        let ms = leaving_ms(0x04, Kind::Forward)
            + reel_ms(hold, &[75.0, 399.0, 1318.0, 31.0])
            + settling_ms(0x63, 0x04)
            + arriving_ms(0x63);
        assert!((ms - 61_560.0).abs() < 20.0, "{ms}");
        let intro = reel_ms(hold, &[75.0, 399.0, 0.0]);
        assert!((intro - 16_257.0).abs() < 50.0, "{intro}");
        let zoom = reel_ms(hold, &[75.0, 399.0, 1318.0, 0.0]);
        assert!((zoom - 60_383.0).abs() < 50.0, "{zoom}");
        // The logos play at their own level, the rest at the music's.
        assert!(!at_music_level("xmt0017.nfx"));
        assert!(at_music_level("xmt0016.nfx"));
        assert!(at_music_level("xmt0018.nfx"));
        assert!(!at_music_level("xnufx.nfx"));
    }

    /// The idle demo's turn to the title (2026-10-20, `street-idle-orig`
    /// runs 1b and 2): the intro, then the zoom onto the billboard, the
    /// title armed 45.59 s after the turn from the title (counts 106,362 to
    /// 110,921) and 45.89 s after it from Main (53,075 to 57,664): the
    /// model 0.10 and 0.14 s short (the intro, the turn's first movie, may
    /// open later than the boot's chained one). A Start during the intro:
    /// the zoom opened 0.08 s later and the title stood 1.43 s after the
    /// press (boot 2), 1.41 s on the idle demo's turn; the model within 60
    /// ms of both (the title's settling ran 0.10 to 0.17 s).
    #[test]
    fn the_title_comes_again_with_the_intro_and_the_zoom() {
        let change = Change::new(0x63, 0x63, Kind::Forward, false, None);
        assert_eq!(reel(&change, true), vec!["xmt0018.nfx", "xmt0015.nfx"]);
        assert_eq!(movie(Kind::Forward, 0x03, 0x63, false), Some("xmt0018.nfx"));
        assert_eq!(whoosh("xmt0018.nfx"), None);
        assert_eq!(whoosh("xmt0015.nfx"), None);
        let tail = settling_ms(0x63, 0x63) + arriving_ms(0x63);
        for (from, measured) in [(0x63, 45_590.0), (0x03, 45_890.0)] {
            let ms = leaving_ms(from, Kind::Forward)
                + reel_ms(holding_ms(0x63, true), &[1318.0, 31.0])
                + tail;
            assert!(
                (ms - measured).abs() < 150.0,
                "{from:#x}: {ms} against {measured}"
            );
        }
        let skipped = reel_ms(START_ZOOM_MS, &[31.0]) + tail;
        for measured in [1430.0, 1410.0] {
            assert!(
                (skipped - measured).abs() < 60.0,
                "{skipped} against {measured}"
            );
        }
    }

    /// Who takes a Start while a change's movies play (`0x2de71c`): the
    /// boot's title only during its intro (index 2; boots 1 and 2 pressed
    /// it in the publisher and the league's notice and nothing changed, and 8
    /// s into the intro the zoom came), a later title's until one Start was
    /// taken (`+0x70`; at the boot a Start in the zoom changed nothing),
    /// the DVD pages' always, the rest never.
    #[test]
    fn the_title_and_the_dvd_pages_take_start() {
        let check = |change: Change, titled: bool, armed: bool, index: usize| {
            let t = Transition {
                phase: Phase::Holding {
                    reel: Reel {
                        boot: change.titles() && !titled,
                        names: reel(&change, titled),
                        index,
                        clip: None,
                        next: None,
                        due: 0.0,
                        ended: None,
                    },
                    change,
                },
                titled,
                armed,
                ..default()
            };
            t.start_check()
        };
        let boot = Change::new(0x04, 0x63, Kind::Forward, false, None);
        assert_eq!(check(boot, false, false, 0), None);
        assert_eq!(check(boot, false, false, 1), None);
        assert_eq!(check(boot, false, false, 2), Some(Skip::Zoom));
        assert_eq!(check(boot, false, false, 3), None);
        let again = Change::new(0x03, 0x63, Kind::Forward, false, None);
        assert_eq!(check(again, true, true, 0), Some(Skip::Zoom));
        assert_eq!(check(again, true, true, 1), Some(Skip::Zoom));
        assert_eq!(check(again, true, false, 1), None);
        let trailer = Change {
            dvd: Some(&["xmt0018.nfx"]),
            ..Change::new(0x66, 0x66, Kind::Forward, false, None)
        };
        assert_eq!(check(trailer, true, false, 0), Some(Skip::End));
        let options = Change::new(0x03, 0x28, Kind::Forward, false, None);
        assert_eq!(check(options, true, false, 0), None);
        // A later title arms its check as its change begins.
        let mut t = Transition {
            titled: true,
            ..default()
        };
        t.begin(again, 0.0);
        assert!(t.armed);
        t.begin(options, 0.0);
        assert!(!t.armed);
    }

    /// The reels: a DVD page's movies into itself (What's New's Play All
    /// the four), the credits' NuFX logo and blur (class 8) or the blur
    /// alone (class 7), and the song held for the title's movies until the
    /// zoom and for a DVD page's.
    #[test]
    fn the_reels_name_their_movies() {
        let all: &'static [&'static str] =
            &["xmt0024.nfx", "xmt0025.nfx", "xmt0026.nfx", "xmt0027.nfx"];
        let play_all = Change {
            dvd: Some(all),
            ..Change::new(0x67, 0x67, Kind::Forward, false, None)
        };
        assert_eq!(reel(&play_all, true), all.to_vec());
        assert_eq!(
            reel(&Change::new(0x09, 0x08, Kind::Forward, false, None), true),
            vec!["xnufx.nfx", "xmtfblur.nfx"]
        );
        assert_eq!(
            reel(&Change::new(0x09, 0x07, Kind::Forward, false, None), true),
            vec!["xmtfblur.nfx"]
        );
        // Back from the credits to Options: the blur back.
        assert_eq!(
            reel(&Change::new(0x07, 0x28, Kind::Back, false, None), true),
            vec!["xmtblur.nfx"]
        );
        let mut t = Transition::default();
        t.begin(play_all, 0.0);
        assert!(t.holds_song());
        let boot = Change::new(0x04, 0x63, Kind::Forward, false, None);
        t.begin(boot, 0.0);
        assert!(t.holds_song());
        t.phase = Phase::Holding {
            change: boot,
            reel: Reel {
                names: BOOT_REEL.to_vec(),
                index: 3,
                clip: None,
                next: None,
                due: 0.0,
                ended: None,
                boot: true,
            },
        };
        assert!(!t.holds_song(), "the song starts with the zoom");
        t.begin(Change::new(0x03, 0x28, Kind::Forward, false, None), 0.0);
        assert!(!t.holds_song());
    }

    /// `FUN_001a19b8`'s reading of the names.
    #[test]
    fn each_movie_starts_its_sound() {
        assert_eq!(whoosh("xmtblur.nfx"), Some((0, 3)));
        assert_eq!(whoosh("xmtfblur.nfx"), Some((0, 3)));
        assert_eq!(whoosh("xmt0000.nfx"), Some((2, 0)));
        assert_eq!(whoosh("xmt0001.nfx"), Some((1, 3)));
        assert_eq!(whoosh("xmt0008.nfx"), Some((2, 0)));
        assert_eq!(whoosh("xmt0011.nfx"), Some((2, 0)));
        assert_eq!(whoosh("xmt0013.nfx"), Some((1, 3)));
        assert_eq!(whoosh("xmt0015.nfx"), None);
        assert_eq!(whoosh("xmt0020.nfx"), Some((2, 0)));
        assert_eq!(whoosh("xlp0005.nfx"), None);
    }

    fn piece(role: Role, x0: f32, x1: f32) -> Piece {
        Piece {
            role,
            ..Piece::new(Handle::default(), Rect::new(x0, 40.0, x1, 80.0))
        }
    }

    /// Main's title slides off at 2.4 px/ms while its rows fade over
    /// 166.7 ms, the poster's city loop staying; Play It's slides in.
    #[test]
    fn the_records_slide_the_title_and_fade_the_rest() {
        let records = leaving(0x03, Kind::Forward).unwrap();
        let mut pieces = vec![
            piece(Role::Title, 60.0, 230.0),
            piece(Role::Widget, 300.0, 400.0),
            piece(Role::Fixed, 265.0, 585.0),
        ];
        pose(&mut pieces, &records, 50.0);
        assert!((pieces[0].rect.min.x - (60.0 - 120.0)).abs() < 1e-3);
        assert!((pieces[1].colour.alpha() - 0.7).abs() < 1e-3);
        assert_eq!(pieces[2].rect.min.x, 265.0);
        assert_eq!(pieces[2].colour.alpha(), 1.0);
        // Off the left edge by 6 once it has travelled, and stays there.
        let mut gone = vec![piece(Role::Title, 60.0, 230.0)];
        pose(&mut gone, &records, 200.0);
        assert!((gone[0].rect.max.x + 6.0).abs() < 1e-3);
        // Main leaving back has none; Play It arrives sliding.
        assert_eq!(leaving(0x03, Kind::Back), None);
        let arriving = arriving(0x05).unwrap();
        let mut title = vec![piece(Role::Title, 60.0, 230.0)];
        pose(&mut title, &arriving, 0.0);
        assert!((title[0].rect.max.x + 6.0).abs() < 1e-3);
        let mut title = vec![piece(Role::Title, 60.0, 230.0)];
        pose(&mut title, &arriving, 266.7);
        assert_eq!(title[0].rect.min.x, 60.0);
    }

    /// A press while the new screen arrives waits, unseen, and reaches it
    /// once it stands; a press while the old screen leaves is not kept, nor
    /// one while the movie plays (boot 2: Down 1.2 s into Main's `xmt0000`
    /// and Main opened on Play It, Cross going to Play It); a Start there
    /// goes to the change's check, here a DVD page's.
    #[test]
    fn presses_wait_for_the_new_screen() {
        let mut app = App::new();
        app.init_resource::<ButtonInput<KeyCode>>()
            .init_resource::<Transition>()
            .add_systems(Update, queue_presses);
        let change = Change::new(0x03, 0x28, Kind::Forward, false, None);
        let press = |app: &mut App, k| {
            let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
            keys.clear();
            keys.press(k);
        };
        let release = |app: &mut App, k| {
            app.world_mut()
                .resource_mut::<ButtonInput<KeyCode>>()
                .release(k);
        };
        app.world_mut().resource_mut::<Transition>().phase = Phase::Leaving { since: 0.0, change };
        press(&mut app, KeyCode::ArrowUp);
        app.update();
        release(&mut app, KeyCode::ArrowUp);
        let playing = |change: Change| Phase::Holding {
            change,
            reel: Reel {
                names: vec!["xmtfblur.nfx"],
                index: 0,
                clip: None,
                next: None,
                due: 0.0,
                ended: None,
                boot: false,
            },
        };
        app.world_mut().resource_mut::<Transition>().phase = playing(change);
        press(&mut app, KeyCode::ArrowLeft);
        app.update();
        release(&mut app, KeyCode::ArrowLeft);
        let t = app.world().resource::<Transition>();
        assert!(t.queue.is_empty(), "dropped under the movie");
        assert_eq!(t.skip, None);
        app.world_mut().resource_mut::<Transition>().phase = Phase::Arriving { since: 0.0, change };
        press(&mut app, KeyCode::ArrowDown);
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(!keys.just_pressed(KeyCode::ArrowDown), "held back");
        assert_eq!(
            app.world().resource::<Transition>().queue,
            vec![Press::Key(KeyCode::ArrowDown)]
        );
        release(&mut app, KeyCode::ArrowDown);
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.world_mut().resource_mut::<Transition>().phase = Phase::Idle;
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(keys.just_pressed(KeyCode::ArrowDown), "replayed");
        assert!(!keys.just_pressed(KeyCode::ArrowUp), "dropped");
        assert!(!keys.just_pressed(KeyCode::ArrowLeft), "dropped");
        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .clear();
        app.update();
        let keys = app.world().resource::<ButtonInput<KeyCode>>();
        assert!(!keys.pressed(KeyCode::ArrowDown), "let go after an update");
        assert!(app.world().resource::<Transition>().queue.is_empty());
        // A DVD page's movie: Start goes to its check, and no further.
        let trailer = Change {
            dvd: Some(&["xmt0018.nfx"]),
            ..Change::new(0x66, 0x66, Kind::Forward, false, None)
        };
        app.world_mut().resource_mut::<Transition>().phase = playing(trailer);
        press(&mut app, KeyCode::Enter);
        app.update();
        let t = app.world().resource::<Transition>();
        assert_eq!(t.skip, Some(Skip::End));
        assert!(t.queue.is_empty());
        assert!(
            !app.world()
                .resource::<ButtonInput<KeyCode>>()
                .just_pressed(KeyCode::Enter)
        );
    }

    /// The model against the runtime's times from the press to the new
    /// screen in state 4 (100 Hz timer): leaving deadline + hold + movie +
    /// settling + arriving deadline, each within a few updates.
    /// The changes with no movie between Main's family whoosh at the
    /// press; those with a movie whoosh by its name.
    #[test]
    fn the_keys_whoosh_the_changes_without_a_movie() {
        for (from, to, kind) in [
            (0x03, 0x05, Kind::Forward),
            (0x03, 0x06, Kind::Forward),
            (0x03, 0x66, Kind::Forward),
            (0x05, 0x69, Kind::Forward),
            (0x66, 0x67, Kind::Forward),
            (0x66, 0x68, Kind::Forward),
            (0x05, 0x03, Kind::Back),
            (0x06, 0x03, Kind::Back),
            (0x66, 0x03, Kind::Back),
            (0x69, 0x05, Kind::Back),
            (0x67, 0x66, Kind::Back),
            (0x68, 0x66, Kind::Back),
        ] {
            assert!(hand_whoosh(from, to, kind), "{from:#x} to {to:#x}");
            assert_eq!(movie(kind, from, to, false), None, "{from:#x} to {to:#x}");
        }
        for (from, to, kind) in [
            (0x03, 0x28, Kind::Forward),
            (0x05, 0x0c, Kind::Forward),
            (0x69, 0x03, Kind::Back),
            (0x28, 0x03, Kind::Back),
        ] {
            assert!(!hand_whoosh(from, to, kind), "{from:#x} to {to:#x}");
        }
        let mut t = Transition::default();
        t.begin(
            Change {
                whoosh: true,
                ..Change::new(0x03, 0x05, Kind::Forward, false, None)
            },
            2.0,
        );
        assert_eq!(t.sound, Some((2.0, 0)));
    }

    #[test]
    fn selects_yes_reaches_main_in_the_measured_times() {
        // Select's Yes in the runtime (2026-10-20, the 100 Hz timer): from
        // the update the answer put the screen in state 2 to Main's state 4.
        // (from, extra hold, measured ms)
        let cases = [
            (0x05, 0.0, 520.0),
            (0x06, 0.0, 520.0),
            (0x66, 0.0, 530.0),
            (0x69, 0.0, 1750.0),
            (0x67, 0.0, 1790.0),
            (0x68, 0.0, 1770.0),
            (0x28, 0.0, 1520.0),
            (0x09, 0.0, 1500.0),
            (0x0a, 0.0, 1490.0),
            (0x14, 0.0, 1510.0),
            (0x61, 0.0, 1500.0),
            (0x37, 0.0, 1500.0),
            (0x1b, 0.0, 1510.0),
            (0x36, 0.0, 1515.0),
            (0x29, 0.0, 1530.0),
            (0x0d, 0.0, 1520.0),
            (0x27, 0.0, 1530.0),
            (0x30, crate::exit_ask::hold_ms(0x30), 1840.0),
            (0x4a, crate::exit_ask::hold_ms(0x4a), 1990.0),
        ];
        for (from, extra, measured) in cases {
            let frames = if movie(Kind::Back, from, 0x03, false).is_some() {
                30.0
            } else {
                0.0
            };
            let movie_ms =
                holding_ms(0x03, frames > 0.0) + extra + frames * 1000.0 / MOVIE_RATE as f32;
            let model = leaving_ms(from, Kind::Back)
                + movie_ms
                + settling_ms(0x03, from)
                + arriving_ms(0x03);
            assert!(
                (model - measured).abs() <= 70.0,
                "{from:#x} to Main: model {model} ms, runtime {measured} ms"
            );
        }
    }

    #[test]
    fn the_steps_add_up_to_the_measured_times() {
        // (from, to, kind, movie frames, measured ms)
        let cases = [
            (0x03, 0x28, Kind::Forward, 9, 930.0),
            (0x28, 0x03, Kind::Back, 30, 1510.0),
            (0x03, 0x05, Kind::Forward, 0, 600.0),
            (0x69, 0x0d, Kind::Forward, 36, 1890.0),
            (0x0d, 0x1b, Kind::Forward, 28, 1480.0),
            (0x1b, 0x1d, Kind::Forward, 17, 1100.0),
            (0x1d, 0x21, Kind::Forward, 25, 1390.0),
            (0x63, 0x03, Kind::Forward, 77, 3110.0),
        ];
        for (from, to, kind, frames, measured) in cases {
            let movie_ms = holding_ms(to, frames > 0) + frames as f32 * 1000.0 / MOVIE_RATE as f32;
            let model = leaving_ms(from, kind) + movie_ms + settling_ms(to, from) + arriving_ms(to);
            assert!(
                (model - measured).abs() <= 70.0,
                "{from:#x} to {to:#x}: model {model} ms, runtime {measured} ms"
            );
        }
    }
}
