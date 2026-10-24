//! Be A Legend's event intros on the game's own layouts: Pick Up Game
//! (class 0x31 = 49, ctor `FUN_002be4a0`, build `FUN_002be518`),
//! Tournament (0x32, `FUN_002d5800`, build `FUN_002d5878`) and Street
//! Challenge (0x33, `FUN_002d14f8`, build `FUN_002d1570`); each
//! `FUN_002d7720(screen, class, 0xe)`, backdrop 14. The trace is in
//! `docs/FRONTEND_RECOVERY.md`, "Be A Legend's event intros".
//!
//! The builds read the current court (game data `+0xcc`) and the career's
//! current event (`+0xf28` bits 2..7): element 2 shows the court's big
//! picture (`XACOURTS` sheet `court`, frame 0, `FUN_00210048(w, 3, court,
//! 0, 0)`), element 0 its name between two stars (`XACOMMON` sheet 0,
//! image 1, frame 2, each 20 beyond the name's half width, middle-anchored:
//! `FUN_002be910`, `FUN_002d5cc0`, `FUN_002d1a18`), element 4 its location
//! (hidden for the region words 2..4), element 1 (a kind-11 text box,
//! `FUN_0023aa68`) the court's story from the screen's table (`0x48e958`,
//! `0x493e58`, `0x492618`) wrapped to the box's 250, and element 3 the
//! progress line. The street challenge adds its rule's name (element 5,
//! `0x492630`) and, on court 10, "Be A Legend" for its title (element 6).

use bevy::prelude::*;
use street_sim::story::{EVENTS, EventKind, Story};

use crate::{
    art::Art,
    look::{self, Align, Look, Piece, fonts},
    setup::Disc,
};

/// The screens' backdrop (`FUN_002d7720(.., 0xe)`).
pub const BACKDROP: usize = 14;
/// The court of Rucker Park '78, whose challenge is Be A Legend's last.
const LEGEND_COURT: u16 = 10;
/// The stars' distance beyond the name's half width (`fVar13 = 20.0`).
const STAR_GAP: f32 = 20.0;

/// The layout class of an event's intro.
pub fn class(ev: usize) -> u32 {
    match EVENTS[ev].kind {
        EventKind::Pickup => 49,
        EventKind::Tournament => 50,
        EventKind::StreetChallenge => 51,
    }
}

/// The intro's title (the layout's kind-35 text: `pickupintro.txt` 0,
/// `tourneyintro.txt` 0, `streetchallengeintro.txt` 0; `FUN_002d1570`
/// sets element 6 to 0x1501 "Be A Legend" on court 10).
pub fn title_id(ev: usize) -> u16 {
    match EVENTS[ev].kind {
        EventKind::Pickup => 0x1300,
        EventKind::Tournament => 0x1400,
        EventKind::StreetChallenge if EVENTS[ev].court == LEGEND_COURT => 0x1501,
        EventKind::StreetChallenge => 0x1500,
    }
}

/// The court's story (element 1): the screen's table at `0x48e958`
/// (0x1302 + court), `0x493e58` (0x1404 + court) or `0x492618` (0x1505 +
/// court).
pub fn story_id(ev: usize) -> u16 {
    let court = EVENTS[ev].court;
    match EVENTS[ev].kind {
        EventKind::Pickup => 0x1302 + court,
        EventKind::Tournament => 0x1404 + court,
        EventKind::StreetChallenge => 0x1505 + court,
    }
}

/// A street challenge's rule (element 5, the table at `0x492630`: 0x1510
/// + court).
pub fn rule_id(ev: usize) -> Option<u16> {
    (EVENTS[ev].kind == EventKind::StreetChallenge).then(|| 0x1510 + EVENTS[ev].court)
}

/// The progress line (element 3). `FUN_002be518`: "Progress: %1d/%2d at
/// %3S" (0x1301) of the event's tally, its target (`FUN_001e7550`) and the
/// court's name. `FUN_002d5878`: the same (0x1401) for a target over 0,
/// else whether it is cleared (`FUN_001e7ff8`: 0x1403, 0x1402).
/// `FUN_002d1570`: for no target whether it is cleared (0x1502, 0x1503),
/// else 0x1504 of the target once cleared (0 before), the target and the
/// name.
pub fn progress_line(look: &Look, story: &Story, ev: usize, court: &str) -> String {
    let row = &EVENTS[ev];
    let target = i32::from(row.target.max(0));
    let tally = i32::from(story.progress[ev]);
    let format = |id: u16, plain: &str, first: i32| {
        look.text_id(id)
            .unwrap_or(plain)
            .replace("%1d", &first.to_string())
            .replace("%2d", &target.to_string())
            .replace("%3S", court)
    };
    let cleared = story.cleared(ev);
    match row.kind {
        EventKind::Pickup => format(0x1301, "Progress: %1d/%2d at %3S", tally),
        EventKind::Tournament if target > 0 => format(0x1401, "Progress: %1d/%2d at %3S", tally),
        EventKind::Tournament if cleared => {
            format(0x1403, "You have completed this tournament.", 0)
        }
        EventKind::Tournament => format(0x1402, "You have not completed this tournament.", 0),
        EventKind::StreetChallenge if target > 0 => format(
            0x1504,
            "Progress: %1d/%2d at %3S",
            if cleared { target } else { 0 },
        ),
        EventKind::StreetChallenge if cleared => {
            format(0x1502, "You have completed this Street Challenge.", 0)
        }
        EventKind::StreetChallenge => {
            format(0x1503, "You have not completed this Street Challenge.", 0)
        }
    }
}

/// `text` broken into lines no wider than `width` in `font`, at spaces (the
/// kind-11 box's layout, `FUN_0023abf8`).
pub fn wrap(font: &street_assets::font::Font, text: &str, width: f32) -> Vec<String> {
    let mut out = Vec::new();
    for para in text.replace("\\n", "\n").split('\n') {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() {
                word.to_owned()
            } else {
                format!("{line} {word}")
            };
            if !line.is_empty() && font.measure(&candidate) as f32 > width {
                out.push(std::mem::take(&mut line));
                line = word.to_owned();
            } else {
                line = candidate;
            }
        }
        out.push(line);
    }
    out
}

/// Where a text element's point and anchor put a line of `width`, the box
/// one text height tall (`FUN_00239e70`): its top-left.
fn anchored(at: Vec2, align: u8, width: f32, height: f32) -> Vec2 {
    at - Vec2::new(
        match align & 3 {
            1 => width / 2.0,
            2 => width,
            _ => 0.0,
        },
        match (align >> 2) & 3 {
            1 => height / 2.0,
            2 => height,
            _ => 0.0,
        },
    )
}

/// The intro's own pieces (the title and the legends come with the
/// layout).
pub fn pieces(
    look: &Look,
    labels: &mut look::Labels,
    images: &mut Assets<Image>,
    art: &Art,
    disc: &Disc,
    story: &Story,
    ev: usize,
) -> Vec<Piece> {
    let mut out = Vec::new();
    let Some(layout) = look.layouts.iter().find(|l| l.class == class(ev)) else {
        return out;
    };
    let element = |id: u32| {
        layout
            .elements
            .iter()
            .find(|e| e.id == id && matches!(e.kind, 3 | 4 | 11))
    };
    let point = |id: u32| {
        element(id)
            .and_then(|e| e.position())
            .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y)))
    };
    let court_index = EVENTS[ev].court;
    let court = disc.courts.get(usize::from(court_index));
    let name = court.map_or_else(String::new, |c| c.name.clone());
    // The picture, centred on element 2.
    if let (Some(at), Some(picture)) = (
        point(2),
        art.courts
            .get(usize::from(court_index))
            .and_then(|c| c.as_ref())
            .and_then(|c| c.picture.clone()),
    ) {
        let size = crate::choosecourt::PICTURE;
        out.push(Piece::new(picture, Rect::from_center_size(at, size)));
    }
    let mut line = |out: &mut Vec<Piece>, id: u32, s: &str| -> Option<(Vec2, f32, f32)> {
        let e = element(id)?;
        let at = point(id)?;
        let font_index = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        let font = look.fonts.get(font_index)?;
        let colour = e.colour().unwrap_or(look::colours::PALE);
        let width = font.measure(s) as f32;
        let height = font.text_height as f32;
        let top_left = anchored(at, e.align, width, height);
        if let Some(p) = labels.text(
            look,
            images,
            font_index,
            s,
            colour,
            None,
            top_left,
            Align::Left,
        ) {
            out.push(p);
        }
        Some((top_left + Vec2::new(width, height) / 2.0, width, height))
    };
    // The name and its stars, each centred 20 beyond the name's half width.
    if let Some((middle, width, _)) = line(&mut out, 0, &name)
        && let Some(pm) = look.prematch.as_ref()
    {
        for x in [
            middle.x + width / 2.0 + STAR_GAP,
            middle.x - width / 2.0 - STAR_GAP,
        ] {
            out.push(Piece::new(
                pm.name_star.clone(),
                Rect::from_center_size(Vec2::new(x, middle.y), pm.name_star_size),
            ));
        }
    }
    if let Some(location) = court.map(|c| c.location.clone()) {
        line(&mut out, 4, &location);
    }
    if let Some(rule) = rule_id(ev).and_then(|id| look.text_id(id))
        && rule != "n/a"
    {
        line(&mut out, 5, rule);
    }
    let progress = progress_line(look, story, ev, &name);
    line(&mut out, 3, &progress);
    // The story in its box: font 1 (prop 13), the box's colour (prop 40),
    // lines a line height apart from the box's top.
    if let (Some(e), Some(at)) = (element(1), point(1))
        && let Some(story_text) = look.text_id(story_id(ev))
        && story_text != "n/a"
    {
        let font_index = e.props.get(&13).map_or(fonts::LIST_SMALL, |&f| f as usize);
        let width = e.size().map_or(250.0, |(w, _)| f32::from(w));
        let colour = e
            .props
            .get(&40)
            .map_or(look::colours::LEGEND, |&c| street_assets::font::argb(c));
        if let Some(font) = look.fonts.get(font_index) {
            let pitch = font.line_height as f32;
            for (k, l) in wrap(font, story_text, width).iter().enumerate() {
                if let Some(p) = labels.text(
                    look,
                    images,
                    font_index,
                    l,
                    colour,
                    None,
                    at + Vec2::new(0.0, pitch * k as f32),
                    Align::Left,
                ) {
                    out.push(p);
                }
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_kind_has_its_layout_and_texts() {
        // Event 0 is NYC Rec Center's pick-up (court 0), 2 Soul in the
        // Hole's tournament (court 3), 12 the first street challenge
        // (court 3, Trick Challenge).
        assert_eq!(class(0), 49);
        assert_eq!(class(2), 50);
        assert_eq!(class(12), 51);
        assert_eq!(story_id(0), 0x1302);
        assert_eq!(story_id(2), 0x1404 + 3);
        assert_eq!(rule_id(0), None);
        assert_eq!(rule_id(12), Some(0x1510 + 3));
        assert_eq!(title_id(0), 0x1300);
        assert_eq!(title_id(2), 0x1400);
        assert_eq!(title_id(12), 0x1500);
        // The '78 challenge (event 10, court 10) is Be A Legend's.
        assert_eq!(title_id(10), 0x1501);
    }

    #[test]
    fn the_progress_line_follows_the_build() {
        let look = Look::empty();
        let mut story = Story::default();
        assert_eq!(
            progress_line(&look, &story, 0, "NYC Rec Center"),
            "Progress: 0/15 at NYC Rec Center"
        );
        story.progress[2] = 17;
        assert_eq!(
            progress_line(&look, &story, 2, "Soul in the Hole"),
            "Progress: 17/25 at Soul in the Hole"
        );
        // A street challenge has no target: whether it is cleared.
        assert_eq!(
            progress_line(&look, &story, 12, "Soul in the Hole"),
            "You have not completed this Street Challenge."
        );
    }
}
